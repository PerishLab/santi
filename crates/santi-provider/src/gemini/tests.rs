use super::*;
use crate::{Event, Item, Receipt};
use futures_util::{StreamExt, stream};
use santi_model::thinking::Continuation;

fn config() -> Config {
    Config {
        provider: "local".into(),
        model: "neon".into(),
        key: "fixture".into(),
        url: "http://localhost/v1beta".into(),
        effort: None,
        ceiling: None,
        bytes: 10000,
        maintenance: 12000,
        headers: vec![],
        outgoing: Headers::default(),
    }
}

fn state(parts: Vec<Value>) -> Continuation {
    let cfg = config();
    Continuation {
        provider: cfg.provider,
        endpoint: cfg.url,
        model: cfg.model,
        calls: parts
            .iter()
            .filter(|part| part.get("functionCall").is_some())
            .count(),
        parts: Some(parts),
        content: None,
    }
}
fn request(input: Vec<Item>) -> Request {
    Request {
        instructions: None,
        model: "neon".into(),
        input,
        tools: None,
        previous: None,
        strand: None,
    }
}

#[test]
fn parallel() {
    let parts = vec![
        json!({"text":"","thoughtSignature":"first"}),
        json!({"functionCall":{"name":"read","args":{"n":1}},"thoughtSignature":"signed"}),
        json!({"functionCall":{"name":"read","id":"provided","args":{"n":2}}}),
    ];
    let persisted = serde_json::to_vec(&state(parts.clone())).unwrap();
    let restored = serde_json::from_slice(&persisted).unwrap();
    let mut req = request(vec![Item::Continuation(restored)]);
    for (id, n) in [("a", 1), ("provided", 2)] {
        req.input.push(Item::Call {
            call: id.into(),
            name: "read".into(),
            raw: json!({"n":n}).to_string(),
        });
    }
    req.input.push(Item::Output {
        call: "provided".into(),
        output: "second".into(),
    });
    assert!(history::contents(&config(), &req, true).is_err());
    req.input.push(Item::Output {
        call: "a".into(),
        output: "first".into(),
    });
    let body = history::contents(&config(), &req, true).unwrap();
    assert_eq!(body[0]["parts"], json!(parts));
    assert_eq!(
        body[1]["parts"][0]["functionResponse"]["response"]["output"],
        "first"
    );
    assert_eq!(body[1]["parts"][1]["functionResponse"]["id"], "provided");
    assert!(body[1]["parts"][0]["functionResponse"].get("id").is_none());
    req.model = "different".into();
    assert!(history::contents(&config(), &req, true).is_err());
}

#[test]
fn legacy() {
    let old:Continuation=serde_json::from_value(json!({"provider":"local","model":"neon","endpoint":"http://localhost/v1beta","calls":0,"content":"legacy"})).unwrap();
    assert!(old.parts.is_none());
    assert!(history::contents(&config(), &request(vec![Item::Continuation(old)]), true).is_err());
}

async fn events(frames: Vec<Value>) -> Vec<Result<Event, String>> {
    let bytes = frames
        .into_iter()
        .flat_map(|frame| format!("data: {frame}\r\n\r\n").into_bytes())
        .map(|b| Ok(bytes::Bytes::from(vec![b])))
        .collect::<Vec<_>>();
    let receipt = Receipt {
        status: 200,
        elapsed: 1,
        headers: vec![],
    };
    super::stream::frames(receipt, stream::iter(bytes), state(vec![]))
        .collect()
        .await
}

#[tokio::test]
async fn signed() {
    let first = json!({"text":"证据"});
    let second = json!({"functionCall":{"name":"read","args":{}},"thoughtSignature":"opaque"});
    let last = json!({"text":"","thoughtSignature":"trailing"});
    let data = vec![
        json!({"responseId":"round","candidates":[{"content":{"parts":[first]}}]}),
        json!({"candidates":[{"content":{"parts":[second,last]},"finishReason":"STOP"}]}),
    ];
    let output = events(data)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(output.iter().any(|e|matches!(e,Event::Retained(s) if s.parts.as_ref()==Some(&vec![first.clone(),second.clone(),last.clone()]))));
    assert!(
        output
            .iter()
            .any(|e| matches!(e,Event::Called(call) if call.name=="read"))
    );
}

#[tokio::test]
async fn interrupted() {
    let call = json!({"functionCall":{"name":"read","args":{}}});
    for data in [
        vec![json!({"candidates":[{"content":{"parts":[call]}}]})],
        vec![json!({"promptFeedback":{"blockReason":"SAFETY"}})],
        vec![json!({"candidates":[{"content":{"parts":[call]},"finishReason":"MAX_TOKENS"}]})],
    ] {
        let output = events(data).await;
        assert!(output.last().unwrap().is_err());
        assert!(
            !output
                .iter()
                .any(|e| matches!(e, Ok(Event::Called(_) | Event::Completed { .. })))
        );
    }
}
