use super::*;
use serde_json::json;

#[test]
fn separated() {
    for instructions in [None, Some(""), Some(" \n\t\u{2003}"), Some(" \n提示🙂\t ")] {
        for history in [
            vec![],
            vec![text("system", "own system"), text("user", "hi🙂")],
        ] {
            for tools in [false, true] {
                check(instructions, history.clone(), tools);
            }
        }
    }
}

fn check(instructions: Option<&str>, history: Vec<Item>, tools: bool) {
    let provider = Chat::new(config());
    let mut input = request(history.clone());
    input.instructions = instructions.map(str::to_owned);
    if !tools {
        input.tools = None;
    }
    let input = provider.prepare(input).unwrap();
    let estimate = provider.estimate(&input).unwrap();
    let sent = body(&config(), input);
    let mut messages = Vec::new();
    let prompt = instructions.filter(|text| !text.trim().is_empty());
    if let Some(prompt) = prompt {
        messages.push(json!({"role": "system", "content": prompt}));
    }
    let remaining = history
        .iter()
        .map(|item| match item {
            Item::Message { role, content } => json!({"role": role, "content": content}),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    messages.extend(remaining.clone());
    assert_eq!(sent["messages"], json!(messages));
    assert_eq!(
        estimate.instructions,
        prompt.map_or(0, |text| text.len()) as i64
    );
    assert_eq!(
        estimate.input,
        serde_json::to_vec(&remaining).unwrap().len() as i64
    );
    assert_eq!(estimate.items, messages.len() as i64);
    assert_eq!(
        estimate.total,
        serde_json::to_vec(&sent).unwrap().len() as i64
    );
    assert_eq!(
        estimate.tools,
        sent.get("tools")
            .map_or(0, |value| serde_json::to_vec(value).unwrap().len()) as i64
    );
    assert_eq!(estimate.estimator, "chat_completions_json_bytes_v1");
}
