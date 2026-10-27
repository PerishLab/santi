use super::*;
use serde_json::{Value, json};

pub fn input(items: Vec<Item>) -> Vec<Value> {
    items
        .into_iter()
        .map(|item| match item {
            Item::Boundary => json!({"kind":"boundary"}),
            Item::Continuation(value) => json!({"kind":"continuation","value":value}),
            Item::Message { role, content } => {
                json!({"kind":"message","role":role,"content":content})
            }
            Item::Call { call, name, raw } => {
                json!({"kind":"call","call":call,"name":name,"raw":raw})
            }
            Item::Output { call, output } => json!({"kind":"output","call":call,"output":output}),
        })
        .collect()
}

pub fn header(value: &Value) -> Value {
    let text = value["content"]
        .as_str()
        .unwrap()
        .split_once("[compact projection]\n")
        .unwrap()
        .1
        .split_once("\n[/compact projection]")
        .unwrap()
        .0;
    serde_json::from_str(text).unwrap()
}
