use serde_json::{Map, Value, json};

use crate::{Item, Request, Tool};

use super::*;

pub(super) fn body(config: &Config, request: Request) -> Value {
    let mut body = Map::from_iter([
        ("model".to_string(), json!(request.model)),
        ("messages".to_string(), messages(&request)),
        ("stream".to_string(), json!(true)),
    ]);

    if let Some(tools) = request.tools {
        body.insert("tools".to_string(), json!(tooled(tools)));
    }
    if let Some(thinking) = config
        .thinking
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        body.insert("thinking".to_string(), json!({ "type": thinking }));
    }
    if let Some(effort) = config
        .effort
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        body.insert("reasoning_effort".to_string(), json!(effort));
    }
    if let Some(ceiling) = config.ceiling {
        body.insert("max_tokens".to_string(), json!(ceiling));
    }

    Value::Object(body)
}

fn messages(request: &Request) -> Value {
    let mut messages = Vec::new();
    if let Some(instructions) = request
        .instructions
        .as_ref()
        .filter(|instructions| !instructions.trim().is_empty())
    {
        messages.push(json!({
            "role": "system",
            "content": instructions,
        }));
    }
    messages.extend(request.input.iter().map(message));
    json!(messages)
}

fn message(item: &Item) -> Value {
    match item {
        Item::Message { role, content } => texted(role, content),
        Item::Call { call, name, raw } => calling(call, name, raw),
        Item::Output { call, output } => outputted(call, output),
    }
}

fn texted(role: &str, content: &str) -> Value {
    json!({ "role": role, "content": content })
}

fn calling(call: &str, name: &str, arguments: &str) -> Value {
    json!({
        "role": "assistant",
        "content": Value::Null,
        "tool_calls": [{
            "id": call,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments,
            },
        }],
    })
}

fn outputted(call: &str, output: &str) -> Value {
    json!({ "role": "tool", "tool_call_id": call, "content": output })
}

fn tooled(tools: Vec<Tool>) -> Vec<Value> {
    tools
        .into_iter()
        .map(|tool| match tool {
            Tool::Function(tool) => json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                },
            }),
        })
        .collect()
}
