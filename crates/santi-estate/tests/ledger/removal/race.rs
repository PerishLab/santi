use super::super::seed::{FIRST, SOUL, SUDO, inbox, seed};
use super::guards::strand;
use santi_estate::{Store, TurnDraft};
use santi_model::{message, turn};
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn admission() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    seed(&store).await;
    let other = Store::open(&path).await.expect("independent connection");
    for index in 0..40 {
        let tag = format!("race_{index}");
        strand(&store, &tag).await;
        let barrier = Arc::new(Barrier::new(2));
        let target = tag.clone();
        let aside = other.clone();
        let ready = barrier.clone();
        let writer = tokio::spawn(async move {
            ready.wait().await;
            if index % 2 == 0 {
                aside
                    .accept_inbox(
                        inbox(
                            &format!("input_{index}"),
                            &target,
                            &message::Content::text("keep"),
                        ),
                        10,
                    )
                    .await
                    .map(|_| ())
            } else {
                aside
                    .create_turn(TurnDraft {
                        tag: &format!("turn_{index}"),
                        strand: &target,
                        trigger: turn::Trigger::StrandSend,
                        from: 0,
                        created: FIRST,
                    })
                    .await
                    .map(|_| ())
            }
        });
        barrier.wait().await;
        let ended = store.end_strand(&tag).await;
        let admitted = writer.await.expect("writer");
        assert!(!(ended.is_ok() && admitted.is_ok()), "both accepted: {tag}");
        assert!(
            ended.is_ok() || admitted.is_ok(),
            "neither operation committed: {tag}: {ended:?} {admitted:?}"
        );
        let live = store.strand(&tag).await.expect("strand").is_some();
        if admitted.is_ok() {
            assert!(live, "accepted work lost: {tag}");
            if index % 2 == 0 {
                assert!(
                    store
                        .receipt(&format!("input_{index}"))
                        .await
                        .expect("receipt")
                        .is_some()
                );
            } else {
                assert!(
                    store
                        .turn(&format!("turn_{index}"))
                        .await
                        .expect("turn")
                        .is_some()
                );
            }
        }
        if ended.is_ok() {
            assert!(!live, "retirement not committed: {tag}");
        }
        assert!(live || ended.is_ok(), "unexplained loss: {tag}");
    }
}

async fn settled(store: &Store, tag: &str) {
    let turn = format!("turn_{tag}");
    store
        .create_turn(TurnDraft {
            tag: &turn,
            strand: tag,
            trigger: turn::Trigger::StrandSend,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    super::super::job::job(store, tag, &turn).await;
    super::guards::finish(store, &turn).await;
    store
        .reconcile_effects(&turn, FIRST)
        .await
        .expect("reconcile");
    store
        .transition_job(
            &format!("job_{tag}"),
            santi_estate::TransitionDraft::new(santi_model::job::State::Succeeded, FIRST),
        )
        .await
        .expect("complete");
    store
        .acknowledge_job(&format!("job_{tag}"), FIRST)
        .await
        .expect("ack");
    store
        .create_capability(santi_estate::CapabilityDraft {
            digest: tag,
            expires: 1000,
            soul: SOUL,
            strand: tag,
            turn: &turn,
            call: &format!("job_call_{tag}"),
            effect: &format!("job_effect_{tag}"),
            created: FIRST,
        })
        .await
        .expect("late submission capability");
}

async fn submit(
    store: &Store,
    strand: &str,
    supervisor: &str,
) -> Result<santi_estate::Prepared, String> {
    store
        .prepare_job(
            strand,
            santi_estate::JobDraft {
                environment: None,
                tag: &format!("late_{strand}"),
                description: "late job",
                command: "true",
                cwd: None,
                timeout_seconds: 30,
                output_limit_bytes: 1024,
                remind_every_seconds: None,
                notify_on_terminal: false,
                request_sha256: "late_digest",
                generation: &format!("generation_{strand}"),
                supervisor_ref: supervisor,
                created: FIRST,
            },
            500,
        )
        .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submission() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    seed(&store).await;
    let other = Store::open(&path).await.expect("independent connection");
    for index in 0..20 {
        let tag = format!("submission_{index}");
        strand(&store, &tag).await;
        settled(&store, &tag).await;
        let barrier = Arc::new(Barrier::new(2));
        let aside = other.clone();
        let target = tag.clone();
        let ready = barrier.clone();
        let writer = tokio::spawn(async move {
            ready.wait().await;
            let supervisor = format!("late_supervisor_{target}");
            submit(&aside, &target, &supervisor).await
        });
        barrier.wait().await;
        let ended = store.end_strand(&tag).await;
        let submitted = writer.await.expect("writer");
        assert!(
            !(ended.is_ok() && submitted.is_ok()),
            "both accepted: {tag}"
        );
        assert!(
            ended.is_ok() || submitted.is_ok(),
            "neither operation committed: {tag}: {ended:?} {submitted:?}"
        );
        if submitted.is_ok() {
            assert!(store.strand(&tag).await.expect("strand").is_some());
            assert!(
                store
                    .job_record(&format!("late_{tag}"))
                    .await
                    .expect("job")
                    .is_some()
            );
            assert!(
                store
                    .end_strand(&tag)
                    .await
                    .expect_err("active job protects")
                    .contains("active job")
            );
        }
    }
}

#[tokio::test]
async fn qualification() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    seed(&store).await;
    let tag = "qualification";
    strand(&store, tag).await;
    settled(&store, tag).await;
    let taken = submit(&store, tag, &format!("supervisor_{tag}"))
        .await
        .expect_err("settled supervisor is taken");
    assert!(taken.contains("supervisor_ref taken"), "{taken}");
    assert!(
        store
            .job_record(&format!("late_{tag}"))
            .await
            .expect("job")
            .is_none(),
        "rejected contender left a job"
    );
    let original = store
        .job_record(&format!("job_{tag}"))
        .await
        .expect("job")
        .expect("settled job");
    assert_eq!(original.job.state, santi_model::job::State::Succeeded);
    assert!(store.strand(tag).await.expect("strand").is_some());
    let santi_estate::Prepared::New(record) =
        submit(&store, tag, &format!("late_supervisor_{tag}"))
            .await
            .expect("independent supervisor is admissible")
    else {
        panic!("late job must be new");
    };
    assert_eq!(record.job.id, format!("late_{tag}"));
    assert_eq!(
        store
            .job_record(&format!("job_{tag}"))
            .await
            .expect("job")
            .expect("settled job")
            .job
            .state,
        santi_model::job::State::Succeeded
    );
    assert!(
        store
            .end_strand(tag)
            .await
            .expect_err("active job protects")
            .contains("active job")
    );
    assert!(store.strand(tag).await.expect("strand").is_some());
    assert!(
        store
            .job_record(&format!("late_{tag}"))
            .await
            .expect("protected job")
            .is_some()
    );
}
