use super::*;
use crate::{Item, Request};
use futures_util::{StreamExt, stream};
use santi_model::thinking::Continuation;

fn config() -> Config {
    Config {
        provider: "deepseek".into(),
        key: "fixture".into(),
        model: "flash".into(),
        url: "http://provider.test/v1".into(),
        thinking: Some("enabled".into()),
        replay: Replay::History,
        effort: None,
        ceiling: None,
        bytes: None,
        maintenance: None,
        headers: Vec::new(),
    }
}

fn state(content: &str, calls: usize) -> Continuation {
    Continuation {
        provider: "deepseek".into(),
        model: "flash".into(),
        endpoint: "http://provider.test/v1".into(),
        content: Some(content.into()),
        calls,
    }
}

fn request(input: Vec<Item>) -> Request {
    Request {
        model: "flash".into(),
        instructions: Some("instruction".into()),
        input,
        tools: Some(vec![crate::Tool::Function(crate::Function {
            name: "probe".into(),
            description: "probe".into(),
            parameters: serde_json::json!({}),
        })]),
        previous: None,
    }
}

fn text(role: &str, content: &str) -> Item {
    Item::Message {
        role: role.into(),
        content: content.into(),
    }
}

fn call(id: &str) -> Item {
    Item::Call {
        call: id.into(),
        name: "probe".into(),
        raw: "{}".into(),
    }
}

#[test]
fn grouping() {
    let provider = Chat::new(config());
    let request = provider
        .prepare(request(vec![
            text("user", "run"),
            Item::Continuation(state("完整\n🙂", 2)),
            text("assistant", "preface"),
            call("one"),
            Item::Output {
                call: "one".into(),
                output: "first".into(),
            },
            call("two"),
            Item::Output {
                call: "two".into(),
                output: "second".into(),
            },
            Item::Continuation(state("finish", 0)),
            text("assistant", "done"),
        ]))
        .unwrap();
    provider.validate(&request).unwrap();
    let estimate = provider.estimate(&request).unwrap();
    let body = body(&config(), request);
    assert_eq!(
        estimate.total as usize,
        serde_json::to_vec(&body).unwrap().len()
    );
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 6);
    assert_eq!(messages[2]["content"], "preface");
    assert_eq!(messages[2]["reasoning_content"], "完整\n🙂");
    assert_eq!(messages[2]["tool_calls"].as_array().unwrap().len(), 2);
    assert_eq!(messages[3]["tool_call_id"], "one");
    assert_eq!(messages[4]["tool_call_id"], "two");
    assert_eq!(messages[5]["reasoning_content"], "finish");
}

#[test]
fn scope() {
    let provider = Chat::new(config());
    let mut input = request(vec![
        Item::Continuation(state("old", 0)),
        text("assistant", "prior answer"),
        text("user", "next"),
        Item::Continuation(state("active", 1)),
        call("one"),
        Item::Output {
            call: "one".into(),
            output: "result".into(),
        },
    ]);
    input.tools = None;
    let input = provider.prepare(input).unwrap();
    provider.validate(&input).unwrap();
    let body = body(&config(), input);
    assert!(body["messages"][1].get("reasoning_content").is_none());
    assert_eq!(body["messages"][3]["reasoning_content"], "active");
}

#[test]
fn provenance() {
    let provider = Chat::new(config());
    for changed in ["provider", "model", "endpoint"] {
        let mut saved = state("original", 1);
        match changed {
            "provider" => saved.provider = "other".into(),
            "model" => saved.model = "other".into(),
            _ => saved.endpoint = "http://other.test".into(),
        }
        let input = request(vec![Item::Continuation(saved), call("one")]);
        assert!(
            provider
                .validate(&input)
                .unwrap_err()
                .contains("another provider, model or endpoint")
        );
    }
}

#[test]
fn legacy() {
    let provider = Chat::new(config());
    let input = provider
        .prepare(request(vec![text("assistant", "old"), call("one")]))
        .unwrap();
    assert!(
        provider
            .validate(&input)
            .unwrap_err()
            .contains("compact the completed legacy history")
    );
    assert!(
        provider.estimate(&input).is_some(),
        "diagnosis must remain available for recovery"
    );
}

#[test]
fn stateless() {
    for disabled in [false, true] {
        let mut config = config();
        if disabled {
            config.thinking = Some("disabled".into());
        } else {
            config.replay = Replay::Stateless;
        }
        let provider = Chat::new(config.clone());
        let input = provider
            .prepare(request(vec![
                Item::Continuation(state("original", 1)),
                call("one"),
            ]))
            .unwrap();
        provider.validate(&input).unwrap();
        let body = body(&config, input);
        assert!(body["messages"][1].get("reasoning_content").is_none());
    }
}

#[tokio::test]
async fn fragments() {
    let raw = concat!(
        "data: {\"id\":\"r\",\"choices\":[{\"delta\":{\"reasoning_content\":\"完整🙂\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"r\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n"
    );
    let chunks = raw
        .as_bytes()
        .chunks(1)
        .map(|chunk| Ok(bytes::Bytes::copy_from_slice(chunk)))
        .collect::<Vec<Result<_, reqwest::Error>>>();
    let receipt = crate::Receipt {
        status: 200,
        elapsed: 0,
        headers: Vec::new(),
    };
    let events = frames(receipt, stream::iter(chunks), state("", 0))
        .collect::<Vec<_>>()
        .await;
    let retained = events
        .into_iter()
        .map(Result::unwrap)
        .find_map(|event| match event {
            crate::Event::Retained(state) => Some(state),
            _ => None,
        })
        .unwrap();
    assert_eq!(retained.content.as_deref(), Some("完整🙂"));
}

#[test]
fn boundaries() {
    let config = config();
    let prefix = vec![
        Item::Continuation(state("original", 2)),
        call("one"),
        Item::Output {
            call: "one".into(),
            output: "result".into(),
        },
    ];
    let active = request(prefix.clone());
    super::replay::validate(&config, &active, false).unwrap();
    assert!(
        super::replay::validate(&config, &active, true)
            .unwrap_err()
            .contains("incomplete")
    );
    let mut crossed = prefix;
    crossed.push(text("user", "next request"));
    assert!(super::replay::validate(&config, &request(crossed), false).is_err());
    let interrupted = request(vec![
        Item::Continuation(state("", 0)),
        text("assistant", "first"),
        Item::Boundary,
        text("assistant", "partial retry"),
    ]);
    assert!(
        super::replay::validate(&config, &interrupted, true)
            .unwrap_err()
            .contains("missing")
    );
    let orphan = request(vec![Item::Output {
        call: "missing".into(),
        output: "result".into(),
    }]);
    assert!(
        super::replay::validate(&config, &orphan, true)
            .unwrap_err()
            .contains("orphan")
    );
}

#[test]
fn presence() {
    for content in [None, Some(String::new())] {
        let mut saved = state("", 1);
        saved.content = content.clone();
        let input = request(vec![
            Item::Continuation(saved),
            call("one"),
            Item::Output {
                call: "one".into(),
                output: "result".into(),
            },
        ]);
        let body = body(&config(), input);
        assert_eq!(
            body["messages"][1]
                .get("reasoning_content")
                .and_then(serde_json::Value::as_str),
            content.as_deref()
        );
    }
}
