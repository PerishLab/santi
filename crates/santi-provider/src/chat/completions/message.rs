use crate::Item;
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct Messages {
    output: Vec<Value>,
    assistant: Option<Value>,
    replies: Vec<Value>,
}

impl Messages {
    pub(super) fn lower(mut self, input: &[Item]) -> Vec<Value> {
        for item in input {
            self.push(item);
        }
        self.flush();
        self.output
    }

    fn push(&mut self, item: &Item) {
        match item {
            Item::Boundary => self.flush(),
            Item::Continuation(state) => {
                self.flush();
                self.assistant = Some(json!({
                    "role": "assistant",
                    "content": null,
                }));
                if let (Some(assistant), Some(content)) =
                    (self.assistant.as_mut(), state.content.as_ref())
                {
                    assistant["reasoning_content"] = json!(content);
                }
            }
            Item::Message { role, content } if role == "assistant" && self.assistant.is_some() => {
                if let Some(assistant) = self.assistant.as_mut() {
                    let text = assistant["content"].as_str().unwrap_or("");
                    assistant["content"] = json!(format!("{text}{content}"));
                }
            }
            Item::Call { call, name, raw } if self.assistant.is_some() => {
                self.call(call, name, raw);
            }
            Item::Output { call, output } if self.assistant.is_some() => {
                self.replies.push(outputted(call, output));
            }
            item => {
                self.flush();
                self.output.push(plain(item));
            }
        }
    }

    fn call(&mut self, call: &str, name: &str, raw: &str) {
        let Some(assistant) = self.assistant.as_mut() else {
            return;
        };
        if assistant.get("tool_calls").is_none() {
            assistant["tool_calls"] = json!([]);
        }
        if let Some(calls) = assistant["tool_calls"].as_array_mut() {
            calls.push(calling(call, name, raw));
        }
    }

    fn flush(&mut self) {
        if let Some(assistant) = self.assistant.take()
            && (assistant.get("tool_calls").is_some()
                || assistant["content"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()))
        {
            self.output.push(assistant);
        }
        self.output.append(&mut self.replies);
    }
}

fn plain(item: &Item) -> Value {
    match item {
        Item::Message { role, content } => json!({ "role": role, "content": content }),
        Item::Call { call, name, raw } => json!({
            "role": "assistant", "content": null,
            "tool_calls": [calling(call, name, raw)],
        }),
        Item::Output { call, output } => outputted(call, output),
        Item::Continuation(_) | Item::Boundary => Value::Null,
    }
}

fn calling(call: &str, name: &str, arguments: &str) -> Value {
    json!({
        "id": call, "type": "function",
        "function": { "name": name, "arguments": arguments },
    })
}

fn outputted(call: &str, output: &str) -> Value {
    json!({ "role": "tool", "tool_call_id": call, "content": output })
}
