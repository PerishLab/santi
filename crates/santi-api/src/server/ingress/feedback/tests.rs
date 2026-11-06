use super::*;
use receipt::reply::Attempt;

fn binding() -> Binding {
    Binding {
        adaptor: "feishu".into(),
        context: json!({"app":"cli_test", "parent":"om_input", "chat":"oc_test"}),
        delay: 3000,
    }
}
fn attempt(target: Option<&str>) -> Attempt {
    Attempt {
        operation: "final".into(),
        uuid: "explicit_uuid".into(),
        owner: Owner::Soul,
        body: render::document("final", false),
        state: State::Dispatching,
        target: target.map(str::to_string),
        result: None,
        resolution: None,
    }
}
fn accepted() -> Value {
    serde_json::from_str(r#"{"feishu":{"action":"im.messages.reply","outcome":"accepted","identity":{"app":"cli_test"},"data":{"message_id":"om_reply","parent_id":"om_input","chat_id":"oc_test","msg_type":"post","sender":{"id":"cli_test","sender_type":"app"}}}}"#).unwrap()
}

fn rejected(state: &str) -> Value {
    json!({"feishu":{"error":{"outcome":state}}})
}

#[test]
fn identity() {
    let binding = binding();
    let create = attempt(None);
    let value = accepted();
    assert_eq!(
        dispatch::classify(&value.to_string(), true, &binding, &create),
        ("accepted", Some("om_reply".into()))
    );
    for (pointer, replacement) in [
        ("/feishu/action", json!("im.messages.edit")),
        ("/feishu/outcome", json!("unknown")),
        ("/feishu/identity/app", json!("cli_foreign")),
        ("/feishu/data/parent_id", json!("om_foreign")),
        ("/feishu/data/chat_id", json!("oc_foreign")),
        ("/feishu/data/msg_type", json!("text")),
        ("/feishu/data/message_id", Value::Null),
        ("/feishu/data/sender/id", json!("cli_foreign")),
        ("/feishu/data/sender/sender_type", json!("user")),
    ] {
        let mut malformed = value.clone();
        *malformed.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(
            dispatch::classify(&malformed.to_string(), true, &binding, &create).0,
            "unknown",
            "{pointer}"
        );
    }
    let mut user = value.clone();
    user["feishu"]["identity"]["user"] = json!("ou_person");
    assert_eq!(
        dispatch::classify(&user.to_string(), true, &binding, &create).0,
        "unknown"
    );
    assert_eq!(
        dispatch::classify("incomplete", true, &binding, &create).0,
        "unknown"
    );
    for state in ["unsent", "rejected", "unknown"] {
        let error = rejected(state);
        assert_eq!(
            dispatch::classify(&error.to_string(), false, &binding, &create).0,
            state
        );
    }
    let mut edited = value;
    edited["feishu"]["action"] = json!("im.messages.edit");
    assert_eq!(
        dispatch::classify(
            &edited.to_string(),
            true,
            &binding,
            &attempt(Some("om_reply"))
        )
        .0,
        "accepted"
    );
    assert_eq!(
        dispatch::classify(
            &edited.to_string(),
            true,
            &binding,
            &attempt(Some("om_foreign"))
        )
        .0,
        "unknown"
    );
}

#[test]
fn marker() {
    let status = render::document("正在处理", true);
    let line = status["content"][0].as_array().unwrap();
    assert_eq!(line.len(), 2);
    assert_eq!(line[0]["tag"], "a");
    assert_eq!(line[0]["href"], render::SOURCE);
    assert!(status.get("title").is_none());
    let formal = render::document("final text", false);
    assert_eq!(formal["content"][0].as_array().unwrap().len(), 1);
    assert!(!formal.to_string().contains(render::SOURCE));
}
