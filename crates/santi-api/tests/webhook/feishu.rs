use super::*;

#[test]
fn challenge() {
    let body =
        format!(r#"{{"challenge":"ch-42","token":"{FEISHU_TOKEN}","type":"url_verification"}}"#);
    let outcome = FeishuAdaptor::configured(None, None)
        .normalize(&HeaderMap::new(), body.as_bytes(), FEISHU_TOKEN, "chat")
        .expect("normalize");
    assert!(
        matches!(outcome, WebhookOutcome::Reply(value) if value == json!({"challenge":"ch-42"}))
    );
}

#[test]
fn token() {
    let body = br#"{"challenge":"ch","token":"wrong","type":"url_verification"}"#;
    let result = FeishuAdaptor::configured(None, None).normalize(
        &HeaderMap::new(),
        body,
        FEISHU_TOKEN,
        "chat",
    );
    assert!(matches!(result, Err(WebhookError::Unauthorized(_))));
}

#[test]
fn encrypted() {
    let plain = feishu_payload("user", "ou_alice", "im.message.receive_v1");
    let body = format!(r#"{{"encrypt":"{}"}}"#, encrypt(&plain));
    let event = expect_event(
        FeishuAdaptor::configured(Some(FEISHU_KEY), Some("ou_alice"))
            .normalize(&HeaderMap::new(), body.as_bytes(), FEISHU_TOKEN, "chat")
            .expect("normalize"),
    );
    assert!(event.in_scope);
    assert_eq!(event.label, "feishu:chat:chat:oc_chat1");
    assert!(event.text.contains("message: om_msg1"));
    assert_eq!(event.delivery.as_deref(), Some("ev_123"));
    let direct = expect_event(
        FeishuAdaptor::configured(None, Some("ou_alice"))
            .normalize(&HeaderMap::new(), plain.as_bytes(), FEISHU_TOKEN, "chat")
            .unwrap(),
    );
    assert_eq!(
        event.metadata.as_ref().unwrap()["fingerprint"],
        direct.metadata.as_ref().unwrap()["fingerprint"]
    );

    assert!(!event.text.contains("top secret"));

    let result = FeishuAdaptor::configured(None, None).normalize(
        &HeaderMap::new(),
        body.as_bytes(),
        FEISHU_TOKEN,
        "chat",
    );
    assert!(matches!(result, Err(WebhookError::Unauthorized(_))));
}

#[test]
fn gates() {
    let bot = feishu_payload("app", "ou_bot", "im.message.receive_v1");
    let event = expect_event(
        FeishuAdaptor::configured(None, None)
            .normalize(&HeaderMap::new(), bot.as_bytes(), FEISHU_TOKEN, "chat")
            .expect("normalize"),
    );
    assert!(!event.in_scope);

    let human = feishu_payload("user", "ou_stranger", "im.message.receive_v1");
    let event = expect_event(
        FeishuAdaptor::configured(None, Some("ou_operator"))
            .normalize(&HeaderMap::new(), human.as_bytes(), FEISHU_TOKEN, "chat")
            .expect("normalize"),
    );
    assert!(!event.in_scope);
}

#[test]
fn verifies() {
    let body = b"{}";
    let timestamp = "1700000000";
    let nonce = "nonce-1";
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(FEISHU_KEY.as_bytes());
    hasher.update(body);
    let signature = hex::encode(hasher.finalize());
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-lark-request-timestamp",
        HeaderValue::from_static(timestamp),
    );
    headers.insert("x-lark-request-nonce", HeaderValue::from_static(nonce));
    headers.insert(
        "x-lark-signature",
        HeaderValue::from_str(&signature).unwrap(),
    );
    assert!(
        FeishuAdaptor::configured(Some(FEISHU_KEY), None)
            .verify(&headers, body, FEISHU_TOKEN)
            .is_ok()
    );
    assert!(matches!(
        FeishuAdaptor::configured(None, None).verify(&headers, body, FEISHU_TOKEN),
        Err(WebhookError::Unauthorized(_))
    ));
}

#[test]
fn ignores() {
    let body = feishu_payload("user", "ou_alice", "im.chat.updated_v1");
    let event = expect_event(
        FeishuAdaptor::configured(None, None)
            .normalize(&HeaderMap::new(), body.as_bytes(), FEISHU_TOKEN, "chat")
            .expect("normalize"),
    );
    assert!(!event.in_scope);
}
