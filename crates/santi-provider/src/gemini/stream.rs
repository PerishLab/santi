use crate::{Call, Event, Receipt, Report, Trace, Usage};
use async_stream::try_stream;
use futures_core::Stream;
use futures_util::StreamExt;
use santi_model::thinking::Continuation;
use serde_json::{Value, json};

#[derive(Default)]
struct Candidate {
    response: Option<String>,
    finish: Option<String>,
    parts: Vec<Value>,
}

impl Candidate {
    fn frame(&mut self, data: &str) -> Result<Vec<Event>, String> {
        let frame: Value =
            serde_json::from_str(data).map_err(|e| format!("malformed Gemini event: {e}"))?;
        if frame.get("error").is_some() {
            return Err("Gemini reported a stream error".into());
        }
        if let Some(reason) = frame
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
        {
            return Err(format!("Gemini blocked prompt: {reason}"));
        }
        let mut events = Vec::new();
        if self.response.is_none() {
            let id = frame["responseId"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            self.response = Some(id.clone());
            events.push(Event::Started { response: Some(id) });
        }
        for c in frame["candidates"].as_array().into_iter().flatten() {
            self.extend(c, &mut events)?;
        }
        let usage = frame.get("usageMetadata").map(|u| Usage {
            input: u["promptTokenCount"].as_u64(),
            cached: u["cachedContentTokenCount"].as_u64(),
            output: u["candidatesTokenCount"].as_u64(),
            reasoning: u["thoughtsTokenCount"].as_u64(),
        });
        if usage.is_some() || self.finish.is_some() || frame["modelVersion"].is_string() {
            events.push(Event::Traced(Trace::Reported(Report {
                status: self.finish.clone(),
                incomplete: self
                    .finish
                    .as_ref()
                    .filter(|s| s.as_str() != "STOP")
                    .cloned(),
                model: frame["modelVersion"].as_str().map(str::to_owned),
                usage,
            })));
        }
        Ok(events)
    }
    fn extend(&mut self, candidate: &Value, events: &mut Vec<Event>) -> Result<(), String> {
        if candidate["index"].as_u64().unwrap_or(0) != 0 {
            return Err("unexpected additional Gemini candidate".into());
        }
        for part in candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(text) = part["text"].as_str().filter(|s| !s.is_empty()) {
                let delta = if part["thought"] == true {
                    Event::Thinking(text.into())
                } else {
                    Event::Text(text.into())
                };
                events.push(delta);
            }
            self.parts.push(part.clone());
        }
        if let Some(reason) = candidate["finishReason"].as_str() {
            self.finish = Some(reason.to_owned())
        }
        Ok(())
    }
    fn finish(self, mut state: Continuation) -> Result<Vec<Event>, String> {
        let response = self
            .response
            .ok_or("Gemini stream ended without a response")?;
        match self.finish.as_deref() {
            Some("STOP") => (),
            Some(reason) => return Err(format!("Gemini stopped with {reason}")),
            None => return Err("Gemini stream ended without finishReason".into()),
        }
        let mut calls = Vec::new();
        for f in self.parts.iter().filter_map(|p| p.get("functionCall")) {
            let name = f["name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .ok_or("Gemini functionCall missing name")?;
            let arguments = f.get("args").cloned().unwrap_or_else(|| json!({}));
            if !arguments.is_object() {
                return Err("invalid Gemini function arguments".into());
            }
            let id = f["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{}-{}", response, calls.len()));
            if calls.iter().any(|c: &Call| c.call == id) {
                return Err("duplicate Gemini function call identifier".into());
            }
            let raw = arguments.to_string();
            let item = json!({
                "type":"function_call", "id":id, "call_id":id, "name":name, "arguments":raw
            });
            calls.push(Call {
                response: response.clone(),
                mark: Some(id.clone()),
                item,
                call: id,
                name: name.into(),
                arguments,
                raw,
            });
        }
        state.calls = calls.len();
        state.parts = Some(self.parts);
        let mut events = vec![Event::Retained(state)];
        if calls.is_empty() {
            events.push(Event::Completed {
                response: Some(response),
            })
        } else {
            events.extend(calls.into_iter().map(Event::Called));
        }
        Ok(events)
    }
}

pub(super) fn frames(
    receipt: Receipt,
    mut chunks: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin + Send + 'static,
    state: Continuation,
) -> impl Stream<Item = Result<Event, String>> + Send + 'static {
    try_stream! {
        yield Event::Traced(Trace::Received(receipt));
        let mut candidate = Candidate::default();
        let mut pending = Vec::new();
        let mut data = Vec::new();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            yield Event::Traced(Trace::Chunk { bytes: chunk.len() });
            pending.extend_from_slice(&chunk);
            for event in drain(&mut pending, &mut data, &mut candidate)? {
                yield event;
            }
        }
        if !pending.is_empty() || !data.is_empty() {
            Err("truncated Gemini SSE frame")?;
        }
        for event in candidate.finish(state)? {
            yield event;
        }
    }
}

fn drain(
    pending: &mut Vec<u8>,
    data: &mut Vec<String>,
    candidate: &mut Candidate,
) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    let mut offset = 0;
    while let Some(length) = pending[offset..].iter().position(|b| *b == b'\n') {
        let line = std::str::from_utf8(&pending[offset..offset + length])
            .map_err(|_| "invalid Gemini SSE UTF-8")?
            .trim_end_matches('\r');
        offset += length + 1;
        if line.is_empty() && !data.is_empty() {
            let frame = data.join("\n");
            data.clear();
            events.extend(candidate.frame(&frame)?);
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value).to_string());
        }
    }
    pending.drain(..offset);
    Ok(events)
}
