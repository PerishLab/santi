use santi_core::{Item, assemble, preview};
use santi_estate::{Call, Compact, Fork, Message, Reply, Store, Strand, Thinking, Turn};
use santi_model::{message, thinking, turn};

mod batches;
mod fixture;
mod reply;
mod speech;
mod timing;
mod value;

const FIRST: &str = "2026-07-28T00:00:00.000Z";
const LATER: &str = "2026-07-28T00:01:00.000Z";

#[tokio::test]
async fn assembles() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(
        temp.path().join("estate.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("open");
    store.seed("soul_test", FIRST).await.expect("seed");
    store
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
        .place(Message {
            tag: "message_first",
            strand: "strand_test",
            actor: message::Role::System,
            sender: "santi",
            kind: message::Kind::Text,
            content: &message::Content::text("run"),
            state: message::State::Fixed,
            request: true,
            created: FIRST,
        })
        .await
        .expect("first");
    store
        .start(Turn {
            tag: "turn_test",
            strand: "strand_test",
            trigger: turn::Trigger::System,
            from: 1,
            created: FIRST,
        })
        .await
        .expect("turn");
    store
        .ponder(Thinking {
            tag: "thinking_test",
            turn: "turn_test",
            response: Some("response_test"),
            created: FIRST,
        })
        .await
        .expect("thinking");
    store
        .refine("thinking_test", None, Some("reasoned"), LATER)
        .await
        .expect("summarize");
    store
        .conclude("thinking_test", thinking::Reason::Called, LATER)
        .await
        .expect("complete thinking");
    store
        .calling(Call {
            tag: "call_test",
            turn: "turn_test",
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: LATER,
        })
        .await
        .expect("call");
    store
        .replying(Reply {
            tag: "result_test",
            call: "call_test",
            output: Some(&serde_json::json!({"stdout": "done"})),
            error: None,
            created: LATER,
        })
        .await
        .expect("reply");
    store
        .place(Message {
            tag: "message_last",
            strand: "strand_test",
            actor: message::Role::Soul,
            sender: "soul_test",
            kind: message::Kind::Text,
            content: &message::Content::text("done"),
            state: message::State::Fixed,
            request: false,
            created: LATER,
        })
        .await
        .expect("last");

    let input = assemble(&store, "strand_test").await.expect("assembly");
    assert_eq!(input.len(), 4);
    check(&input[0], "user", "[message message_first]\nrun");
    let Item::Call { call, name, raw } = &input[1] else {
        panic!("expected call");
    };
    assert_eq!(call, "call_test");
    assert_eq!(name, "shell");
    assert_eq!(raw, r#"{"command":"true"}"#);
    let Item::Output { call, output } = &input[2] else {
        panic!("expected output");
    };
    assert_eq!(call, "call_test");
    assert!(output.contains("done"));
    let facts: serde_json::Value = serde_json::from_str(output).expect("result facts");
    assert_eq!(facts["timing"]["called_at"], LATER);
    assert_eq!(facts["timing"]["replied_at"], LATER);
    assert_eq!(facts["timing"]["call_to_result_ms"], 0);
    check(&input[3], "assistant", "done");
    let child = store
        .fork(Fork {
            tag: "strand_child",
            parent: "strand_test",
            at: 5,
            inference: None,
            created: LATER,
        })
        .await
        .expect("fork");
    let inherited = assemble(&store, &child.id).await.expect("fork input");
    assert_eq!(inherited.len(), 4);
    let Item::Output { output: replay, .. } = &inherited[2] else {
        panic!("expected inherited output");
    };
    assert_eq!(replay, output);
    check(&inherited[0], "user", "[message message_first]\nrun");
    check(&inherited[3], "assistant", "done");
    assert_eq!(
        store
            .message("message_first")
            .await
            .expect("lookup")
            .expect("original")
            .message
            .content
            .rendered(),
        "run"
    );
    let first = identity(&input[0]);
    let last = "message_last";

    let report = store
        .preview("compact_preview", "strand_test", first, last)
        .await
        .expect("preview");
    let metadata = serde_json::json!({
        "schema": "santi.compact_capsule.v1",
        "before": { "total": 1200 },
        "forecast": {
            "authoritative": false,
            "basis": "precommit_preview",
            "after": { "total": 480 },
            "ratio": 0.4,
        },
        "budget": { "bytes": 4000 },
    });
    let projected = preview(
        &store,
        "strand_test",
        santi_core::Preview {
            report: &report,
            summary: "summary",
            metadata: &metadata,
        },
    )
    .await
    .expect("preview assembly");
    compacted(&projected);
    let manual = serde_json::json!({
        "schema": "santi.compact_estimate.v1",
        "operation": "manual_compact",
        "before": metadata["before"],
        "forecast": metadata["forecast"],
        "budget": metadata["budget"],
    });
    let plain = preview(
        &store,
        "strand_test",
        santi_core::Preview {
            report: &report,
            summary: "summary",
            metadata: &manual,
        },
    )
    .await
    .expect("manual preview assembly");
    compacted(&plain);
    store
        .condense(Compact {
            tag: "compact_test",
            strand: "strand_test",
            first,
            last,
            summary: "summary",
            metadata: Some(&metadata),
            expected: None,
            created: LATER,
        })
        .await
        .expect("compact");
    compacted(
        &assemble(&store, "strand_test")
            .await
            .expect("compact assembly"),
    );
}

fn identity(item: &Item) -> &str {
    let Item::Message { content, .. } = item else {
        panic!("expected message");
    };
    content
        .strip_prefix("[message ")
        .and_then(|text| text.split_once("]\n"))
        .expect("message identity")
        .0
}

fn check(item: &Item, expected_role: &str, expected_content: &str) {
    let Item::Message { role, content } = item else {
        panic!("expected message");
    };
    assert_eq!(role, expected_role);
    assert_eq!(content, expected_content);
}

fn compacted(items: &[Item]) {
    assert_eq!(items.len(), 1);
    let Item::Message { role, content } = &items[0] else {
        panic!("expected compact message");
    };
    assert_eq!(role, "system");
    assert!(content.contains("[compact projection]"));
    assert!(content.contains("<compact_summary>\nsummary\n</compact_summary>"));
    let header = content
        .strip_prefix("[compact projection]\n")
        .and_then(|content| content.split_once("\n[/compact projection]"))
        .map(|(header, _)| header)
        .expect("compact projection header");
    let header: serde_json::Value = serde_json::from_str(header).expect("header json");
    assert_eq!(
        header["schema"],
        "santi.compact_projection.visible_header.v1"
    );
    assert_eq!(header["covered_message_range"]["first"], "message_first");
    assert_eq!(header["covered_message_range"]["last"], "message_last");
    let estimate = &header["context_estimate"];
    assert_eq!(estimate["pre_total_bytes"], 1200);
    assert_eq!(estimate["budget_input_bytes"], 4000);
    assert!(estimate.get("post_total_bytes").is_none());
    assert!(estimate.get("ratio").is_none());
    assert_eq!(estimate["forecast"]["authoritative"], false);
    assert_eq!(estimate["forecast"]["basis"], "precommit_preview");
    assert_eq!(estimate["forecast"]["post_total_bytes"], 480);
    assert_eq!(estimate["forecast"]["ratio"], 0.4);
}

#[path = "materials/main.rs"]
mod materials;
