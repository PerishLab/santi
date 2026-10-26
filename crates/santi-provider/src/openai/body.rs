use serde_json::{Map, Value, json};

use crate::{Item, Request, Tool};

use super::*;

pub(super) fn body(config: &Config, request: Request) -> Value {
    let mut body = Map::from_iter([
        ("model".to_string(), json!(request.model)),
        ("input".to_string(), input(&request)),
        ("stream".to_string(), json!(true)),
        ("store".to_string(), json!(false)),
        (
            "stream_options".to_string(),
            json!({
                "include_obfuscation": false
            }),
        ),
    ]);

    if let Some(instructions) = request
        .instructions
        .filter(|instructions| !instructions.trim().is_empty())
    {
        body.insert("instructions".to_string(), json!(instructions));
    }
    if let Some(tools) = request.tools {
        body.insert("tools".to_string(), json!(tooled(tools)));
    }
    if let Some(previous) = request.previous {
        body.insert("previous_response_id".to_string(), json!(previous));
    }
    if let Some(reasoning) = options(config) {
        body.insert("reasoning".to_string(), reasoning);
    }
    if let Some(ceiling) = config.ceiling {
        body.insert("max_output_tokens".to_string(), json!(ceiling));
    }

    Value::Object(body)
}

fn options(config: &Config) -> Option<Value> {
    let mut reasoning = Map::new();
    if let Some(effort) = config
        .effort
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        reasoning.insert("effort".to_string(), json!(effort));
    }
    if let Some(summary) = config
        .summary
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        reasoning.insert("summary".to_string(), json!(summary));
    }
    if reasoning.is_empty() {
        None
    } else {
        Some(Value::Object(reasoning))
    }
}

fn input(request: &Request) -> Value {
    let items = request
        .input
        .iter()
        .filter(|item| !matches!(item, Item::Continuation(_) | Item::Boundary))
        .map(item)
        .collect::<Vec<_>>();
    json!(items)
}

fn item(item: &Item) -> Value {
    match item {
        Item::Continuation(_) | Item::Boundary => Value::Null,
        Item::Message { role, content } => message(role, content),
        Item::Call { call, name, raw } => called(call, name, raw),
        Item::Output { call, output } => outputted(call, output),
    }
}

fn message(role: &str, content: &str) -> Value {
    let mime = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };
    json!({
        "role": role,
        "content": [{ "type": mime, "text": content }],
    })
}

fn called(call: &str, name: &str, arguments: &str) -> Value {
    json!({
        "type": "function_call",
        "call_id": call,
        "name": name,
        "arguments": arguments,
    })
}

fn outputted(call: &str, output: &str) -> Value {
    json!({ "type": "function_call_output", "call_id": call, "output": output })
}

fn tooled(tools: Vec<Tool>) -> Vec<Value> {
    tools
        .into_iter()
        .map(|tool| match tool {
            Tool::Function(tool) => json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            }),
        })
        .collect()
}
