mod batch;
mod entries;
mod faults;
mod forks;
mod freshness;
mod lifetime;
mod support;

use santi_estate::{
    Arrival, Call, Drain, Effect, Message, Opening, Reply, Store, Strand, Thinking,
};
use santi_model::{message, strand, thinking, turn};

const FIRST: &str = "2026-07-28T00:00:00.000Z";
const LATER: &str = "2026-07-28T00:01:00.000Z";

#[tokio::test]
async fn timeline() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = super::support::bootstrap(&path).await;
    store.seed("soul_test", FIRST).await.expect("seed");
    let strand = store
        .weave(Strand {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    store
        .accept(
            Arrival {
                tag: "inbox_test",
                strand: &strand.id,
                kind: message::Kind::Text,
                content: &message::Content::text("hello"),
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .expect("inbox");
    let Opening::Started(opened) = store
        .drain(Drain {
            turn: "turn_test",
            strand: &strand.id,
            trigger: turn::Trigger::System,
            actor: "santi",
            created: LATER,
        })
        .await
        .expect("drain")
    else {
        panic!("expected started turn");
    };
    let span = store
        .ponder(Thinking {
            tag: "thinking_test",
            turn: &opened.turn.id,
            response: Some("response_test"),
            created: LATER,
        })
        .await
        .expect("thinking");
    store
        .conclude(&span.id, thinking::Reason::Called, LATER)
        .await
        .expect("finish thinking");
    let call = store
        .calling(Call {
            tag: "call_test",
            turn: &opened.turn.id,
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: LATER,
        })
        .await
        .expect("call");
    store
        .stage(Effect {
            tag: "effect_test",
            turn: &opened.turn.id,
            call: Some(&call.id),
            kind: "shell",
            metadata: None,
            created: LATER,
        })
        .await
        .expect("effect");
    store
        .replying(Reply {
            tag: "result_test",
            call: &call.id,
            output: Some(&serde_json::json!({"stdout": "done"})),
            error: None,
            created: LATER,
        })
        .await
        .expect("reply");

    assert_eq!(store.events(&strand.id).await.expect("events").len(), 1);
    assert_eq!(
        store.thinkings(&strand.id).await.expect("thinking").len(),
        1
    );
    assert_eq!(
        store.thought(&opened.turn.id).await.expect("thought").len(),
        1
    );
    assert_eq!(store.calls(&strand.id).await.expect("calls").len(), 1);
    assert_eq!(
        store.called(&opened.turn.id).await.expect("called").len(),
        1
    );
    assert_eq!(store.results(&strand.id).await.expect("results").len(), 1);
    assert_eq!(
        store.replied(&opened.turn.id).await.expect("replied").len(),
        1
    );
    assert_eq!(store.effects(&strand.id).await.expect("effects").len(), 1);
    let entries = store.entries(&strand.id).await.expect("entries");
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].kind, strand::Target::Message);
    assert_eq!(entries[1].kind, strand::Target::Thinking);
    assert_eq!(entries[2].kind, strand::Target::Call);
    assert_eq!(entries[3].kind, strand::Target::Result);
    assert_eq!(entries[3].seq, 4);

    let snapshot = store
        .snapshot(&strand.id)
        .await
        .expect("snapshot")
        .expect("held");
    assert_eq!(snapshot.messages.len(), 1);
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.turns.len(), 1);
    assert_eq!(snapshot.thinking.len(), 1);
    assert_eq!(snapshot.calls.len(), 1);
    assert_eq!(snapshot.results.len(), 1);
    assert_eq!(snapshot.effects.len(), 1);
    assert!(snapshot.compacts.is_empty());
    assert!(snapshot.errors.is_empty());
    assert!(store.snapshot("missing").await.expect("missing").is_none());

    drop(store);
    let store = Store::open(path).await.expect("open again");
    let snapshot = store
        .snapshot(&strand.id)
        .await
        .expect("snapshot again")
        .expect("held");
    assert_eq!(snapshot.events[0].action, "insert");
    assert_eq!(snapshot.calls[0].id, "call_test");
    assert_eq!(snapshot.results[0].id, "result_test");

    for index in 0..17 {
        let text = match index {
            15 => "decision-bearing change: freeze the public execution tail",
            16 => "Clock: 2026-08-05T00:16:00.000Z",
            _ => "repeated activity: bounded preflight",
        };
        store
            .place(Message {
                tag: &format!("message_tail_{index}"),
                strand: &strand.id,
                actor: message::Role::System,
                sender: "clock",
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                state: message::State::Fixed,
                request: false,
                created: LATER,
            })
            .await
            .expect("tail message");
    }

    let tail = store
        .tail(&strand.id)
        .await
        .expect("execution tail")
        .expect("held");
    assert_eq!(tail.records.len(), 16);
    assert!(tail.omitted);
    assert!(
        tail.records
            .windows(2)
            .all(|pair| pair[0].sequence > pair[1].sequence)
    );
    assert!(
        tail.records[0]
            .detail
            .contains("Clock: 2026-08-05T00:16:00.000Z")
    );
    assert!(
        tail.records[1]
            .detail
            .contains("decision-bearing change: freeze the public execution tail")
    );
    assert_eq!(
        tail.records
            .iter()
            .filter(|record| record
                .detail
                .contains("repeated activity: bounded preflight"))
            .count(),
        14
    );
    assert!(tail.records.iter().all(|record| !record.truncated));
    assert!(store.tail("missing").await.expect("missing tail").is_none());
}
