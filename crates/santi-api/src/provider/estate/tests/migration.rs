use super::*;

const PROFILE: &str = r#"
[operator]
kind = "chat_completions"
identity = "deepseek"
api_key = "fixture-one"
base_url = "https://fixture.invalid/v1"
thinking = "enabled"
replay = "history"
bytes = 1000
seal = "dededededededededededededededededededededededededededededededede"
request_headers = { x-private = "private-one", x-opencode-session = "first-{{strand.id}}" }
"#;

fn imported(profile: &str) -> Catalog<'static> {
    Catalog {
        custody: Box::leak(Box::new(toml::from_str(profile).unwrap())),
    }
}

#[test]
fn migration() {
    let resolver = imported(PROFILE);
    let mut rows = graph();
    let D::Access { protocol, .. } = &mut rows[3].definition else {
        unreachable!()
    };
    *protocol = Protocol::Chat;
    let direct = Declaration {
        scenario: Some("gateway/family/review".into()),
        ..Default::default()
    };
    let admitted = resolver
        .selecting(&[(&direct, Origin::Strand)], &rows)
        .unwrap();
    let exported = serde_json::to_string(&admitted).unwrap();
    for private in [
        "first-",
        "private-one",
        "fixture-one",
        "dededede",
        "strand.id",
    ] {
        assert!(!exported.contains(private));
    }
    let old = resolver.custody["operator"]
        .resolve("deepseek", "offered")
        .unwrap();
    let metadata = crate::provider::build(old).unwrap().metadata();
    let state = Continuation {
        provider: metadata.provider.to_string(),
        model: metadata.model.clone(),
        endpoint: "https://fixture.invalid/v1".into(),
        calls: 0,
        content: Some("prior reasoning".into()),
        parts: None,
    };
    let request = Request {
        model: metadata.model,
        instructions: None,
        strand: None,
        tools: None,
        previous: None,
        input: vec![
            Item::Continuation(state.clone()),
            Item::Message {
                role: "assistant".into(),
                content: "answer".into(),
            },
        ],
    };
    let restarted = imported(&PROFILE.replace("fixture-one", "fixture-two"))
        .build(&admitted)
        .unwrap();
    let mut request = restarted.prepare(request).unwrap();
    restarted.validate(&request).unwrap();
    assert!(matches!(&request.input[0], Item::Continuation(held) if held == &state));
    for field in ["provider", "endpoint", "model", "protocol"] {
        let mut different = state.clone();
        match field {
            "provider" => different.provider.push('x'),
            "endpoint" => different.endpoint.push('x'),
            "model" => different.model.push('x'),
            _ => different.parts = Some(vec![]),
        }
        request.input[0] = Item::Continuation(different);
        assert!(restarted.validate(&request).is_err());
    }
    for (from, to) in [
        ("1000", "1100"),
        ("history", "stateless"),
        ("x-private", "x-other"),
        ("deepseek", "other"),
        ("first-", "second-"),
        ("private-one", "private-two"),
    ] {
        assert!(
            imported(&PROFILE.replace(from, to))
                .build(&admitted)
                .is_err()
        );
    }
}

#[tokio::test]
async fn wire() {
    use axum::{Json, Router, routing::post};
    use futures_util::StreamExt;
    use serde_json::Value;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sent, mut bodies) = tokio::sync::mpsc::channel(2);
    let app = Router::new().route("/v1/chat/completions", post(move |Json(body): Json<Value>| {
        let sent = sent.clone();
        async move {
            sent.send(body).await.unwrap();
            ([("content-type", "text/event-stream")], concat!(
                "data: {\"id\":\"r\",\"choices\":[{\"delta\":{\"reasoning_content\":\"prior reasoning\"}}]}\n\n",
                "data: {\"id\":\"r\",\"choices\":[{\"delta\":{\"content\":\"answer\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"))
        }
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let config = PROFILE.replace("https://fixture.invalid/v1", &url);
    let legacy = imported(&config.replace("identity = \"deepseek\"", ""));
    let old = crate::provider::build(
        legacy.custody["operator"]
            .resolve("deepseek", "offered")
            .unwrap(),
    )
    .unwrap();
    let mut request = Request {
        strand: Some("fixture".into()),
        model: "offered".into(),
        instructions: None,
        tools: None,
        previous: None,
        input: vec![Item::Message {
            role: "user".into(),
            content: "question".into(),
        }],
    };
    let mut stream = old.stream(request.clone()).await.unwrap();
    let mut retained = None;
    while let Some(event) = stream.next().await {
        if let santi_provider::Event::Retained(state) = event.unwrap() {
            retained = Some(state);
        }
    }
    bodies.recv().await.unwrap();
    request.input.push(Item::Continuation(retained.unwrap()));
    request.input.push(Item::Message {
        role: "assistant".into(),
        content: "answer".into(),
    });
    let mut rows = graph();
    let D::Access {
        endpoint, protocol, ..
    } = &mut rows[3].definition
    else {
        unreachable!()
    };
    *protocol = Protocol::Chat;
    *endpoint = url;
    let catalog = imported(&config);
    let direct = Declaration {
        scenario: Some("gateway/family/review".into()),
        ..Default::default()
    };
    let effective = catalog
        .selecting(&[(&direct, Origin::Strand)], &rows)
        .unwrap();
    let migrated = catalog.build(&effective).unwrap();
    drop(migrated.stream(request).await.unwrap());
    let body = bodies.recv().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(
        messages.last().unwrap()["reasoning_content"],
        "prior reasoning"
    );
    server.abort();
}

#[test]
fn signature() {
    let raw = PROFILE
        .replace("chat_completions", "gemini_generate_content")
        .replace("deepseek", "legacy-gemini")
        .replace("thinking = \"enabled\"", "")
        .replace("replay = \"history\"", "");
    let catalog = imported(&raw);
    let direct = Declaration {
        scenario: Some("gateway/family/review".into()),
        ..Default::default()
    };
    let selected = catalog
        .selecting(&[(&direct, Origin::Strand)], &graph())
        .unwrap();
    let provider = catalog.build(&selected).unwrap();
    let mut request = Request {
        model: "offered".into(),
        strand: None,
        instructions: None,
        tools: None,
        previous: None,
        input: vec![Item::Continuation(Continuation {
            provider: "legacy-gemini".into(),
            model: "offered".into(),
            endpoint: "https://fixture.invalid/v1".into(),
            calls: 0,
            content: None,
            parts: Some(vec![
                serde_json::json!({"text":"answer","thoughtSignature":"legacy-signature"}),
            ]),
        })],
    };
    provider.validate(&request).unwrap();
    let Item::Continuation(state) = &mut request.input[0] else {
        unreachable!()
    };
    state.endpoint.push_str("/other");
    assert!(provider.validate(&request).is_err());
}

#[test]
fn staging() {
    let custody = Default::default();
    let catalog = Catalog { custody: &custody };
    let declaration = Declaration {
        scenario: Some("gateway/family/review".into()),
        ..Default::default()
    };
    assert!(
        catalog
            .selecting(&[(&declaration, Origin::Service)], &[])
            .is_err()
    );
    assert!(
        catalog
            .selecting(&[(&declaration, Origin::Service)], &graph())
            .is_err()
    );
}
