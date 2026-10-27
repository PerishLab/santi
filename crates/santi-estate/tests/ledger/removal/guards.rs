use super::super::seed::{FIRST, LATER, SOUL, SUDO, inbox, seed};
use santi_estate::{DrainDraft, Opening, Store, StrandDraft, TurnDraft};
use santi_model::{effect, job, message, receipt, turn};

pub(super) async fn strand(store: &Store, tag: &str) {
    store
        .create_strand(StrandDraft {
            tag,
            soul: SOUL,
            label: None,
            parent: None,
            fork: None,
            created: FIRST,
        })
        .await
        .expect("strand");
}

pub(super) async fn finish(store: &Store, tag: &str) {
    store
        .finish_turn(santi_estate::CompletionDraft {
            turn: tag,
            reply: None,
            provider: "test",
            model: "test",
            response: None,
            occurred: LATER,
        })
        .await
        .expect("finish");
}

#[tokio::test]
async fn pending() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    seed(&store).await;
    strand(&store, "pending").await;
    store
        .accept_inbox(
            inbox("input", "pending", &message::Content::text("keep me")),
            10,
        )
        .await
        .expect("accept");
    let refusal = store
        .end_strand("pending")
        .await
        .expect_err("pending protects");
    assert!(refusal.contains("pending input input"), "{refusal}");
    assert!(store.strand("pending").await.expect("strand").is_some());
    let Opening::Started(started) = store
        .drain_turn(DrainDraft {
            turn: "drained",
            strand: "pending",
            trigger: turn::Trigger::StrandSend,
            actor: "santi",
            created: LATER,
        })
        .await
        .expect("drain")
    else {
        panic!("pending input survived");
    };
    assert_eq!(started.drained[0].text, "keep me");
    let refusal = store
        .end_strand("pending")
        .await
        .expect_err("driving protects");
    assert!(refusal.contains("unfinished receipt input"), "{refusal}");
    finish(&store, "drained").await;
    assert_eq!(
        store
            .receipt("input")
            .await
            .expect("receipt")
            .expect("live")
            .state,
        receipt::State::Completed
    );
    store.end_strand("pending").await.expect("idle");
    assert!(store.end_strand("pending").await.is_err());
}

#[tokio::test]
async fn active() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    seed(&store).await;
    strand(&store, "work").await;
    store
        .create_turn(TurnDraft {
            tag: "turn_work",
            strand: "work",
            trigger: turn::Trigger::StrandSend,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    let refusal = store.end_strand("work").await.expect_err("turn protects");
    assert!(refusal.contains("running turn turn_work"), "{refusal}");
    super::super::job::job(&store, "work", "turn_work").await;
    finish(&store, "turn_work").await;
    let refusal = store.end_strand("work").await.expect_err("effect protects");
    assert!(
        refusal.contains("unsettled effect job_effect_work"),
        "{refusal}"
    );
    store
        .dispatch_effect("job_effect_work", LATER)
        .await
        .expect("dispatch");
    store
        .unknown_effect("job_effect_work", "result unavailable", LATER)
        .await
        .expect("unknown");
    assert!(
        store
            .end_strand("work")
            .await
            .expect_err("unknown protects")
            .contains("unsettled effect")
    );
    store
        .settle_effect(
            "job_effect_work",
            effect::Outcome::Applied,
            "operator verified",
            LATER,
        )
        .await
        .expect("resolve");
    for state in [
        job::State::Submitting,
        job::State::Accepted,
        job::State::Running,
        job::State::Cancelling,
    ] {
        store
            .transition_job("job_work", santi_estate::TransitionDraft::new(state, LATER))
            .await
            .expect("state");
        let refusal = store.end_strand("work").await.expect_err("job protects");
        assert!(refusal.contains("active job job_work"), "{refusal}");
        assert!(store.job_record("job_work").await.expect("job").is_some());
    }
    store
        .transition_job(
            "job_work",
            santi_estate::TransitionDraft::new(job::State::Succeeded, LATER),
        )
        .await
        .expect("complete");
    assert!(
        store
            .end_strand("work")
            .await
            .expect_err("result protects")
            .contains("unacknowledged job job_work")
    );
    store.acknowledge_job("job_work", LATER).await.expect("ack");
    let keys = store.end_strand("work").await.expect("idle");
    assert!(keys.contains(&"job_work".to_string()));
    assert!(keys.contains(&"stamp_work".to_string()));
    assert!(
        store
            .job_record("job_work")
            .await
            .expect("ended job")
            .is_none()
    );
}
