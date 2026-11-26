mod timeout;

use santi_estate::{Arrival, Call, Effect, Redemption, Store, Strand, Turn};
use santi_model::{effect, message, receipt, turn};

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const FIRST: &str = "2026-07-28T00:00:00.000Z";
const LATER: &str = "2026-07-28T00:01:00.000Z";

#[tokio::test]
async fn lifecycle() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
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
    let inbox = store
        .accept(
            Arrival {
                tag: "inbox_test",
                strand: &strand.id,
                kind: message::Kind::Text,
                content: &message::Content::text("run a tool"),
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .expect("inbox");
    let turn = store
        .start(Turn {
            tag: "turn_test",
            strand: &strand.id,
            trigger: turn::Trigger::Send,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    store
        .advance(santi_estate::Receipt {
            inbox: &inbox.id,
            state: receipt::State::Driving,
            turn: Some(&turn.id),
            incident: None,
            occurred: FIRST,
        })
        .await
        .expect("receipt");
    let call = store
        .calling(Call {
            tag: "call_test",
            turn: &turn.id,
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: FIRST,
        })
        .await
        .expect("call");
    let redeemable = store
        .calling(Call {
            tag: "call_redeemed",
            turn: &turn.id,
            tool: "shell",
            arguments: &serde_json::json!({"command": "printf done"}),
            created: FIRST,
        })
        .await
        .expect("redeemed call");
    let redeemed = store
        .stage(Effect {
            tag: "effect_redeemed",
            turn: &turn.id,
            call: Some(&redeemable.id),
            kind: "shell",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("redeemed effect");
    assert!(
        store
            .redeem(
                &redeemed.id,
                Redemption {
                    result: "result_early",
                    call: &redeemable.id,
                    output: Some(&serde_json::json!({"stdout": "done"})),
                    error: None,
                    outcome: effect::Outcome::Applied,
                    occurred: LATER,
                },
            )
            .await
            .is_err()
    );
    assert!(store.reply("result_early").await.expect("early").is_none());
    store
        .dispatching(&redeemed.id, LATER)
        .await
        .expect("dispatch redeemed");
    let reply = store
        .redeem(
            &redeemed.id,
            Redemption {
                result: "result_redeemed",
                call: &redeemable.id,
                output: Some(&serde_json::json!({"stdout": "done"})),
                error: None,
                outcome: effect::Outcome::Applied,
                occurred: LATER,
            },
        )
        .await
        .expect("redeem");
    assert_eq!(reply.output, Some(serde_json::json!({"stdout": "done"})));
    let redeemed = store
        .effect(&redeemed.id)
        .await
        .expect("redeemed")
        .expect("status");
    assert_eq!(redeemed.effect.result.as_deref(), Some("result_redeemed"));
    assert_eq!(
        redeemed.effect.state,
        effect::State::Settled(effect::Outcome::Applied)
    );
    let prepared = store
        .stage(Effect {
            tag: "effect_test",
            turn: &turn.id,
            call: Some(&call.id),
            kind: "shell",
            metadata: Some(&serde_json::json!({"scope": "external"})),
            created: FIRST,
        })
        .await
        .expect("prepare");
    assert_eq!(prepared.state, effect::State::Prepared);
    let status = store
        .effect(&prepared.id)
        .await
        .expect("effect")
        .expect("status");
    assert_eq!(status.receipts, vec!["inbox_test"]);
    let receipt = store
        .receipt(&inbox.id)
        .await
        .expect("receipt")
        .expect("status");
    assert_eq!(receipt.effects.len(), 2);
    assert!(
        receipt
            .effects
            .iter()
            .any(|effect| effect.id == "effect_test")
    );

    let rejected = store
        .redeem(
            &prepared.id,
            Redemption {
                result: "result_rejected",
                call: &call.id,
                output: None,
                error: Some("dispatch rejected"),
                outcome: effect::Outcome::Unapplied,
                occurred: LATER,
            },
        )
        .await
        .expect("reject");
    assert_eq!(rejected.error.as_deref(), Some("dispatch rejected"));
    let settled = store
        .effect(&prepared.id)
        .await
        .expect("effect")
        .expect("status");
    assert_eq!(
        settled.effect.state,
        effect::State::Settled(effect::Outcome::Unapplied)
    );

    store
        .stage(Effect {
            tag: "effect_prepared",
            turn: &turn.id,
            call: None,
            kind: "test",
            metadata: None,
            created: LATER,
        })
        .await
        .expect("second");
    store
        .stage(Effect {
            tag: "effect_dispatching",
            turn: &turn.id,
            call: None,
            kind: "test",
            metadata: None,
            created: LATER,
        })
        .await
        .expect("third");
    store
        .dispatching("effect_dispatching", LATER)
        .await
        .expect("dispatch third");
    store.reconcile(&turn.id, LATER).await.expect("reconcile");
    assert_eq!(
        store
            .effect("effect_prepared")
            .await
            .expect("prepared")
            .expect("status")
            .effect
            .state,
        effect::State::Settled(effect::Outcome::Unapplied)
    );
    assert_eq!(
        store
            .effect("effect_dispatching")
            .await
            .expect("dispatching")
            .expect("status")
            .effect
            .state,
        effect::State::Unknown
    );
    assert!(
        store
            .settle("effect_dispatching", effect::Outcome::Unapplied, " ", LATER)
            .await
            .is_err()
    );
    let settled = store
        .settle(
            "effect_dispatching",
            effect::Outcome::Unapplied,
            "operator checked",
            LATER,
        )
        .await
        .expect("settle")
        .expect("status");
    assert_eq!(
        settled.effect.state,
        effect::State::Settled(effect::Outcome::Unapplied)
    );
    assert!(
        store
            .settle(
                "effect_dispatching",
                effect::Outcome::Applied,
                "second guess",
                LATER,
            )
            .await
            .is_err()
    );

    drop(store);
    let store = Store::open(path).await.expect("open again");
    assert_eq!(
        store
            .receipt(&inbox.id)
            .await
            .expect("receipt again")
            .expect("status")
            .effects
            .len(),
        4
    );
}
