use super::*;
use santi_core::message;
use santi_estate::{CapabilityDraft, InboxDraft, JobDraft, Prepared, TransitionDraft};

async fn terminal(
    store: &santi_core::Store,
    strand: &santi_core::strand::Strand,
    suffix: &str,
    state: job::State,
) -> String {
    let capability = format!("capability_{suffix}");
    store
        .create_capability(CapabilityDraft {
            digest: &capability,
            expires: 10_000,
            soul: &strand.soul,
            strand: &strand.id,
            turn: "turn_http",
            call: "call_http",
            effect: "effect_http",
            created: "2026-07-28T00:00:00.000Z",
        })
        .await
        .expect("capability");
    let tag = format!("job_{suffix}");
    let request = format!("request_{suffix}");
    let stamp = format!("stamp_{suffix}");
    let supervisor = format!("santi-{suffix}.service");
    let Prepared::New(record) = store
        .prepare_job(
            &capability,
            JobDraft {
                environment: None,
                tag: &tag,
                description: "terminal notification probe",
                command: "true",
                cwd: None,
                timeout_seconds: 30,
                output_limit_bytes: 1024,
                remind_every_seconds: None,
                notify_on_terminal: true,
                request_sha256: &request,
                generation: &stamp,
                supervisor_ref: &supervisor,
                created: "2026-07-28T00:00:00.000Z",
            },
            0,
        )
        .await
        .expect("prepare")
    else {
        panic!("new job");
    };
    store
        .accept_job(&record.job.id, "2026-07-28T00:00:00.000Z")
        .await
        .expect("accept");
    store
        .transition_job(
            &tag,
            TransitionDraft {
                reason: Some("observed terminal"),
                exit_code: Some(1),
                ..TransitionDraft::new(state, "2026-07-28T00:01:00.000Z")
            },
        )
        .await
        .expect("terminal");
    tag
}

#[tokio::test]
async fn notification() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (service, strand, store) = fixture::prepare(
        &temp,
        Arc::new(FakeSupervisor {
            launches: Mutex::new(0),
        }),
    )
    .await;
    let states = [
        job::State::Succeeded,
        job::State::Failed,
        job::State::TimedOut,
        job::State::Cancelled,
        job::State::Unknown,
    ];
    for (index, state) in states.iter().enumerate() {
        let tag = terminal(&store, &strand, &format!("terminal_{index}"), state.clone()).await;
        service
            .ack(&strand.soul, &tag)
            .await
            .expect("ack after delivery");
        let record = store.job_record(&tag).await.expect("record").expect("job");
        assert_eq!(&record.job.state, state);
        assert!(record.terminal_notified.is_some());
        assert!(record.job.acknowledged.is_some());
        assert!(
            store
                .unnotified(10)
                .await
                .expect("unnotified")
                .iter()
                .all(|held| held.job.id != tag)
        );
    }
    assert_eq!(
        store.inboxes(&strand.id).await.expect("inbox").len(),
        states.len()
    );
}

#[tokio::test]
async fn optin() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (service, strand, store) = fixture::prepare(
        &temp,
        Arc::new(FakeSupervisor {
            launches: Mutex::new(0),
        }),
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-santi-job-capability",
        fixture::CAPABILITY.parse().expect("capability header"),
    );
    let (_, Json(created)) = Jobs(service)
        .create(
            headers,
            CreateJobRequest {
                description: "notify flag".to_string(),
                command: "true".to_string(),
                cwd: None,
                timeout_seconds: Some(30),
                output_limit_bytes: Some(1024),
                remind_every_seconds: None,
                notify_on_terminal: Some(true),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("create failed: {}", error.message()));
    assert!(
        store
            .job_record(&created.job.id)
            .await
            .expect("record")
            .expect("job")
            .notify_on_terminal
    );
    assert_eq!(created.job.origin.strand, strand.id);
}

#[tokio::test]
async fn refuses() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (service, strand, store) = fixture::prepare(
        &temp,
        Arc::new(FakeSupervisor {
            launches: Mutex::new(0),
        }),
    )
    .await;
    let tag = terminal(&store, &strand, "capacity", job::State::Succeeded).await;
    for index in 0..500 {
        let inbox = format!("inbox_{index}");
        let content = message::Content::text("pending input");
        store
            .accept_inbox(
                InboxDraft {
                    tag: &inbox,
                    strand: &strand.id,
                    kind: message::Kind::Text,
                    content: &content,
                    source: None,
                    created: "2026-07-28T00:00:00.000Z",
                },
                1000,
            )
            .await
            .expect("inbox");
    }
    let error = service
        .ack(&strand.soul, &tag)
        .await
        .expect_err("ack must refuse while notice cannot be queued");
    assert!(error.contains("not yet durably recorded"));
    let record = store.job_record(&tag).await.expect("record").expect("job");
    assert_eq!(record.job.state, job::State::Succeeded);
    assert!(record.terminal_notified.is_none());
    assert!(record.job.acknowledged.is_none());
    assert_eq!(store.inboxes(&strand.id).await.expect("inbox").len(), 500);
}
