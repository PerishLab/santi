use super::Config;
use crate::{Item, Request};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
struct Exchange<'a> {
    functions: Vec<&'a Value>,
    ids: Vec<&'a str>,
    replies: BTreeMap<&'a str, Value>,
    retained: bool,
}
impl Exchange<'_> {
    fn finish(&mut self, out: &mut Vec<Value>, complete: bool) -> Result<(), String> {
        if complete
            && (self.ids.len() != self.functions.len() || self.replies.len() != self.ids.len())
        {
            return Err(
                "incomplete Gemini tool exchange; retain all calls and results together".into(),
            );
        }
        let mut parts = Vec::new();
        for (f, id) in self.functions.iter().zip(&self.ids) {
            if let Some(value) = self.replies.remove(id) {
                let mut response = json!({"name":f["name"],"response":value});
                if let Some(id) = f["id"].as_str().filter(|id| !id.is_empty()) {
                    response["id"] = json!(id);
                }
                parts.push(json!({"functionResponse":response}));
            }
        }
        if !parts.is_empty() {
            out.push(json!({"role":"user","parts":parts}));
        }
        *self = Self::default();
        Ok(())
    }
}
pub(super) fn contents(cfg: &Config, req: &Request, complete: bool) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut group = Exchange::default();
    for item in &req.input {
        group.push(cfg, &req.model, item, &mut out)?;
    }
    group.finish(&mut out, complete)?;
    Ok(out)
}

impl<'a> Exchange<'a> {
    fn push(
        &mut self,
        cfg: &Config,
        model: &str,
        item: &'a Item,
        out: &mut Vec<Value>,
    ) -> Result<(), String> {
        match item {
            Item::Continuation(s) => {
                self.finish(out, true)?;
                if s.provider != cfg.provider || s.endpoint != cfg.url || s.model != model {
                    return Err(
                        "Gemini history belongs to another profile, endpoint or model".into(),
                    );
                }
                let parts = s
                    .parts
                    .as_ref()
                    .ok_or("Gemini history is missing original ordered parts")?;
                self.functions = parts.iter().filter_map(|p| p.get("functionCall")).collect();
                if self.functions.len() != s.calls {
                    return Err("Gemini retained call count disagrees with parts".into());
                }
                self.retained = true;
                out.push(json!({"role":"model","parts":parts}));
            }
            Item::Boundary => self.finish(out, true)?,
            Item::Message { role, .. } if role == "assistant" => {
                if !self.retained {
                    return Err("original Gemini model parts missing from selected history".into());
                }
            }
            Item::Message { role, content } => {
                self.finish(out, true)?;
                if !matches!(role.as_str(), "user" | "system") {
                    return Err(format!("unexpected Gemini input role {role}"));
                }
                let text = json!({"text":content});
                out.push(json!({"role":"user","parts":[text]}));
            }
            Item::Call { call, name, raw } => {
                let f = self
                    .functions
                    .get(self.ids.len())
                    .ok_or("Gemini tool call has no retained functionCall")?;
                if self.ids.contains(&call.as_str()) {
                    return Err("duplicate Gemini historical tool call identifier".into());
                }
                let args: Value = serde_json::from_str(raw)
                    .map_err(|_| "invalid Gemini historical tool arguments")?;
                if f["name"] != name.as_str()
                    || f.get("args").cloned().unwrap_or_else(|| json!({})) != args
                    || f["id"]
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .is_some_and(|id| id != call)
                {
                    return Err("Gemini historical tool call disagrees with retained parts".into());
                }
                self.ids.push(call);
            }
            Item::Output { call, output } => {
                if !self.ids.contains(&call.as_str()) || self.replies.contains_key(call.as_str()) {
                    return Err("orphan or duplicate Gemini function result".into());
                }
                let value = serde_json::from_str::<Value>(output)
                    .ok()
                    .filter(Value::is_object)
                    .unwrap_or_else(|| json!({"output":output}));
                self.replies.insert(call, value);
            }
        }
        Ok(())
    }
}
