use super::support::*;
use santi_estate::{MessageDraft, ThinkingDraft};
use santi_model::{message, strand};
use serde_json::to_value;

fn record(kind: strand::Target, tag: &str, seq: i64) -> strand::Entry {
    strand::Entry {
        strand: "strand_test".into(),
        kind,
        target: tag.into(),
        seq,
        created: NOW.into(),
    }
}

#[tokio::test]
async fn mixed() {
    let seat = Seat::new().await;
    let mut expected = Vec::new();
    let mut sequence = 1;
    for index in 0..140 {
        let tag = format!("thinking_{:04}", 1000 - index);
        seat.store
            .create_thinking(ThinkingDraft {
                tag: &tag,
                turn: "turn_test",
                response: None,
                created: NOW,
            })
            .await
            .unwrap();
        expected.push(record(strand::Target::Thinking, &tag, sequence));
        seat.pair(index).await;
        expected.push(record(
            strand::Target::ToolCall,
            &format!("call_{:04}", 1000 - index),
            sequence + 1,
        ));
        expected.push(record(
            strand::Target::ToolResult,
            &format!("result_{:04}", 1000 - index),
            sequence + 2,
        ));
        sequence += 3;
        if index == 0 {
            seat.entry(
                "tool_call",
                seat.key("ToolCall", "call_1000").await,
                sequence,
            )
            .await;
            expected.push(record(strand::Target::ToolCall, "call_1000", sequence));
            sequence += 1;
            seat.core
                .set(
                    "Strand",
                    seat.key("Strand", "strand_test").await,
                    &[("next", &sequence.to_string())],
                )
                .await
                .unwrap();
        }
    }
    seat.store
        .place(MessageDraft {
            tag: "message_last",
            strand: "strand_test",
            actor: message::Role::System,
            actor_id: "test",
            kind: message::Kind::Text,
            content: &message::Content::text("工具🙂"),
            state: message::State::Fixed,
            request: false,
            created: NOW,
        })
        .await
        .unwrap();
    expected.push(record(strand::Target::Message, "message_last", sequence));
    sequence += 1;
    seat.entry(
        "tool_result",
        seat.key("ToolResult", "result_1000").await,
        sequence,
    )
    .await;
    expected.push(record(strand::Target::ToolResult, "result_1000", sequence));
    assert_eq!(
        to_value(seat.store.entries("strand_test").await.unwrap()).unwrap(),
        to_value(expected).unwrap()
    );
    assert_eq!(
        seat.store.entries("missing").await.unwrap_err(),
        "strand not found"
    );
}

#[tokio::test]
async fn empty() {
    let seat = Seat::new().await;
    assert!(seat.store.entries("strand_test").await.unwrap().is_empty());
}

#[tokio::test]
async fn invalid() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.damage(
        "PRAGMA ignore_check_constraints = ON; UPDATE strand_strandentry SET target_type = 'opaque' WHERE target_type = 'tool_call'",
    )
    .await;
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        "unknown strand target opaque"
    );
}

#[tokio::test]
async fn field() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    let row = seat
        .core
        .one(&keel::form("StrandEntry").when("target_type", keel::Op::Eq, "tool_call"))
        .await
        .unwrap()
        .unwrap();
    seat.damage("UPDATE strand_strandentry SET call_id = NULL WHERE target_type = 'tool_call'")
        .await;
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        format!("{} missing call", row.key())
    );
}

#[tokio::test]
async fn expired() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    assert_eq!(seat.store.entries("strand_test").await.unwrap().len(), 2);
    let key = seat.key("ToolCall", "call_1000").await;
    seat.damage("UPDATE turn_toolcall SET expires_at = 1").await;
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        format!("ToolCall {key} missing")
    );
    seat.damage("UPDATE soul_strand SET expires_at = 1").await;
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        "strand not found"
    );
}

#[tokio::test]
async fn covered() {
    let seat = Seat::new().await;
    for tag in ["message_first", "message_last"] {
        seat.store
            .place(MessageDraft {
                tag,
                strand: "strand_test",
                actor: message::Role::System,
                actor_id: "test",
                kind: message::Kind::Text,
                content: &message::Content::text("history"),
                state: message::State::Fixed,
                request: false,
                created: NOW,
            })
            .await
            .unwrap();
        if tag == "message_first" {
            seat.pair(0).await;
        }
    }
    seat.store
        .create_compact(santi_estate::CompactDraft {
            tag: "compact_test",
            strand: "strand_test",
            first: "message_first",
            last: "message_last",
            summary: "summary",
            metadata: None,
            expected: None,
            created: NOW,
        })
        .await
        .unwrap();
    seat.damage("UPDATE strand_strandentry SET call_id = 900000 WHERE target_type = 'tool_call'")
        .await;
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        "ToolCall 900000 missing"
    );
}
