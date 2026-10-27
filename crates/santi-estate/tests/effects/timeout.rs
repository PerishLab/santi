use super::*;
use santi_estate::ReplyDraft;

#[tokio::test]
async fn uncertain() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .unwrap();
    store.seed("soul_test", FIRST).await.unwrap();
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
        .unwrap();
    store
        .create_turn(TurnDraft {
            tag: "turn_test",
            strand: "strand_test",
            trigger: turn::Trigger::StrandSend,
            from: 0,
            created: FIRST,
        })
        .await
        .unwrap();
    for call in ["call_test", "call_other"] {
        store
            .create_call(CallDraft {
                tag: call,
                turn: "turn_test",
                tool: "shell",
                arguments: &serde_json::json!({"command":"sleep 30"}),
                created: FIRST,
            })
            .await
            .unwrap();
    }
    store
        .prepare_effect(EffectDraft {
            tag: "effect_test",
            turn: "turn_test",
            call: Some("call_test"),
            kind: "shell",
            metadata: None,
            created: FIRST,
        })
        .await
        .unwrap();
    let output = serde_json::json!({"timed_out":true,"stdout":"partial"});
    let draft = ReplyDraft {
        tag: "result_test",
        call: "call_test",
        output: Some(&output),
        error: None,
        created: LATER,
    };
    assert!(store.uncertain("effect_test", draft).await.is_err());
    assert!(store.reply("result_test").await.unwrap().is_none());
    store.dispatch_effect("effect_test", LATER).await.unwrap();
    assert!(
        store
            .uncertain(
                "effect_test",
                ReplyDraft {
                    call: "call_other",
                    ..draft
                }
            )
            .await
            .is_err()
    );
    assert!(store.reply("result_test").await.unwrap().is_none());
    let reply = store.uncertain("effect_test", draft).await.unwrap();
    assert_eq!(reply.output, Some(output));
    assert!(reply.error.is_none());
    let effect = store.effect("effect_test").await.unwrap().unwrap().effect;
    assert_eq!(effect.state, effect::State::Unknown);
    assert!(effect.settled.is_none());
    assert_eq!(effect.result.as_deref(), Some("result_test"));
}
