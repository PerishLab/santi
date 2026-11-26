use santi_model::{message, strand, stream, turn};

#[test]
fn nested() {
    let payload = stream::Payload::Message(message::Beat::Delta {
        message: "m1".to_string(),
        turn: "t1".to_string(),
        role: message::Role::Soul,
        text: "chunk".to_string(),
    });
    let value = serde_json::to_value(&payload).expect("serialize");
    assert_eq!(value["type"], "message");
    assert_eq!(value["beat"], "delta");
    assert_eq!(value["turn"], "t1");
    assert_eq!(value["text"], "chunk");
    let back: stream::Payload = serde_json::from_value(value).expect("deserialize");
    assert!(matches!(
        back,
        stream::Payload::Message(message::Beat::Delta { .. })
    ));

    let payload = stream::Payload::Turn(turn::Beat::Completed {
        turn: "t1".to_string(),
        label: None,
        text: None,
    });
    let value = serde_json::to_value(&payload).expect("serialize");
    assert_eq!(value["type"], "turn");
    assert_eq!(value["beat"], "completed");
    assert_eq!(value["turn"], "t1");
    assert!(value.get("label").is_none());

    let value = serde_json::to_value(&stream::Payload::Open).expect("serialize");
    assert_eq!(value["type"], "open");
}

#[test]
fn clipping() {
    let content = format!("{}{}", "é".repeat(2_000), "\\\"\n".repeat(2_000));
    let record = stream::Record::project(
        1,
        "2026-08-05T00:00:00.000Z".to_string(),
        strand::Target::Message,
        &content,
    )
    .expect("project");
    assert!(record.truncated);
    assert!(record.detail.len() <= stream::DETAIL);
    assert!(std::str::from_utf8(record.detail.as_bytes()).is_ok());
    assert_eq!(
        record.bytes,
        serde_json::to_string(&content).unwrap().len() as u64
    );
}

#[test]
fn ceiling() {
    let content = "\\\"\n".repeat(4_000);
    let records = (0..stream::RECORDS)
        .map(|index| {
            stream::Record::project(
                i64::try_from(index).unwrap(),
                "2026-08-05T00:00:00.000Z".to_string(),
                strand::Target::Result,
                &content,
            )
            .unwrap()
        })
        .collect();
    let tail = stream::Tail::newest(records, true);
    tail.bounded().expect("bounded response");
    assert!(serde_json::to_vec(&tail).unwrap().len() <= stream::RESPONSE);
}

#[test]
fn rendering() {
    use santi_model::render;
    let payload = stream::Payload::Render(render::Beat::Updated {
        view: render::Updated {
            strand: "s1".into(),
            kind: render::Kind::Skill,
            updated: "now".into(),
        },
    });
    let legacy = serde_json::json!({"type":"material", "beat":"updated", "material":{"strand":"s1", "kind":"skill", "updated":"now"}});
    assert_eq!(serde_json::to_value(&payload).unwrap(), legacy);
    assert!(matches!(
        serde_json::from_value::<stream::Payload>(legacy).unwrap(),
        stream::Payload::Render(_)
    ));
    let view = render::View {
        strand: "s1".into(),
        kind: render::Kind::Skill,
        mime: "text/markdown; charset=utf-8".into(),
        text: "guide".into(),
        updated: "now".into(),
    };
    assert_eq!(
        serde_json::to_value(view).unwrap(),
        serde_json::json!({"strand":"s1", "kind":"skill", "content_type":"text/markdown; charset=utf-8", "text":"guide", "updated":"now"})
    );
}
