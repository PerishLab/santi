use super::super::seed::{FIRST, LATER, SOUL, SUDO, inbox, seed};
use santi_estate::{Drain, Opening, Store, Strand, Turn};
use santi_model::{effect, job, message, receipt, turn};

pub(super) async fn strand(store: &Store, tag: &str) {
    store
        .weave(Strand {
            tag,
            soul: SOUL,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
}

pub(super) async fn finish(store: &Store, tag: &str) {
    store
        .finish(santi_estate::Finish {
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
        .accept(
            inbox("input", "pending", &message::Content::text("keep me")),
            10,
        )
        .await
        .expect("accept");
    let refusal = store.retire("pending").await.expect_err("pending protects");
    assert!(refusal.contains("pending input input"), "{refusal}");
    assert!(store.strand("pending").await.expect("strand").is_some());
    let Opening::Started(started) = store
        .drain(Drain {
            turn: "drained",
            strand: "pending",
            trigger: turn::Trigger::Send,
            actor: "santi",
            created: LATER,
        })
        .await
        .expect("drain")
    else {
        panic!("pending input survived");
    };
    assert_eq!(started.drained[0].text, "keep me");
    let refusal = store.retire("pending").await.expect_err("driving protects");
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
    store.retire("pending").await.expect("idle");
    assert!(store.retire("pending").await.is_err());
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
        .start(Turn {
            tag: "turn_work",
            strand: "work",
            trigger: turn::Trigger::Send,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    let refusal = store.retire("work").await.expect_err("turn protects");
    assert!(refusal.contains("running turn turn_work"), "{refusal}");
    super::super::job::job(&store, "work", "turn_work").await;
    finish(&store, "turn_work").await;
    let refusal = store.retire("work").await.expect_err("effect protects");
    assert!(
        refusal.contains("unsettled effect job_effect_work"),
        "{refusal}"
    );
    store
        .dispatching("job_effect_work", LATER)
        .await
        .expect("dispatch");
    store
        .suspect("job_effect_work", "result unavailable", LATER)
        .await
        .expect("unknown");
    assert!(
        store
            .retire("work")
            .await
            .expect_err("unknown protects")
            .contains("unsettled effect")
    );
    store
        .settle(
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
            .transit("job_work", santi_estate::Transition::new(state, LATER))
            .await
            .expect("state");
        let refusal = store.retire("work").await.expect_err("job protects");
        assert!(refusal.contains("active job job_work"), "{refusal}");
        assert!(store.tracked("job_work").await.expect("job").is_some());
    }
    store
        .transit(
            "job_work",
            santi_estate::Transition::new(job::State::Succeeded, LATER),
        )
        .await
        .expect("complete");
    assert!(
        store
            .retire("work")
            .await
            .expect_err("result protects")
            .contains("unacknowledged job job_work")
    );
    store.ack("job_work", LATER).await.expect("ack");
    let (ended, keys) = store.retire("work").await.expect("idle");
    assert_eq!(ended.jobs, ["job_work"]);
    assert!(keys.contains(&"job_work".to_string()));
    assert!(keys.contains(&"stamp_work".to_string()));
    assert!(
        store
            .tracked("job_work")
            .await
            .expect("ended job")
            .is_none()
    );
}
