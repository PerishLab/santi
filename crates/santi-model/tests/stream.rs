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
fn execution_record_clips_utf8_and_wire_bytes() {
    let content = format!("{}{}", "é".repeat(2_000), "\\\"\n".repeat(2_000));
    let record = stream::ExecutionRecord::project(
        1,
        "2026-08-05T00:00:00.000Z".to_string(),
        strand::Target::Message,
        &content,
    )
    .expect("project");
    assert!(record.detail_truncated);
    assert!(record.detail.len() <= stream::EXECUTION_TAIL_DETAIL_LIMIT_BYTES);
    assert!(std::str::from_utf8(record.detail.as_bytes()).is_ok());
    assert_eq!(
        record.detail_bytes,
        serde_json::to_string(&content).unwrap().len() as u64
    );
}

#[test]
fn execution_tail_success_body_has_a_hard_ceiling() {
    let content = "\\\"\n".repeat(4_000);
    let records = (0..stream::EXECUTION_TAIL_RECORD_LIMIT)
        .map(|index| {
            stream::ExecutionRecord::project(
                i64::try_from(index).unwrap(),
                "2026-08-05T00:00:00.000Z".to_string(),
                strand::Target::ToolResult,
                &content,
            )
            .unwrap()
        })
        .collect();
    let tail = stream::ExecutionTail::newest(records, true);
    tail.ensure_response_bound().expect("bounded response");
    assert!(
        serde_json::to_vec(&tail).unwrap().len() <= stream::EXECUTION_TAIL_RESPONSE_LIMIT_BYTES
    );
}
