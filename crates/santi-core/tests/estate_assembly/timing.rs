use super::*;

#[tokio::test]
async fn retains() {
    let temp = tempfile::tempdir().expect("temp");
    let database = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
    store.seed("soul_test", FIRST).await.expect("seed");
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    store
        .create_turn(TurnDraft {
            tag: "turn_test",
            strand: "strand_test",
            trigger: turn::Trigger::System,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    for (call, result, error) in [
        ("call_ok", "result_ok", None),
        (
            "call_failed",
            "result_failed",
            Some("failed before dispatch"),
        ),
    ] {
        store
            .create_call(CallDraft {
                tag: call,
                turn: "turn_test",
                tool: "shell",
                arguments: &serde_json::json!({"command": "retained history, never executed"}),
                created: FIRST,
            })
            .await
            .expect("call");
        store
            .create_reply(ReplyDraft {
                tag: result,
                call,
                output: error
                    .is_none()
                    .then_some(&serde_json::json!({"retained": true})),
                error,
                created: LATER,
            })
            .await
            .expect("reply");
    }
    drop(store);
    let store = Store::open(&database).await.expect("reopen");
    let input = provider_input(&store, "strand_test")
        .await
        .expect("assembly");
    let outputs = input
        .iter()
        .filter_map(|item| match item {
            Item::Output { output, .. } => Some(output),
            _ => None,
        })
        .map(|output| serde_json::from_str::<serde_json::Value>(output).expect("facts"))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0]["ok"], true);
    assert_eq!(outputs[1]["ok"], false);
    assert_eq!(outputs[1]["error"], "failed before dispatch");
    for output in outputs {
        assert_eq!(output["timing"]["called_at"], FIRST);
        assert_eq!(output["timing"]["replied_at"], LATER);
        assert_eq!(output["timing"]["call_to_result_ms"], 60000);
    }
    let retained = store
        .reply("result_failed")
        .await
        .expect("lookup")
        .expect("retained");
    assert_eq!(retained.created, LATER);
    assert_eq!(retained.error.as_deref(), Some("failed before dispatch"));
    assert!(retained.output.is_none());
}
