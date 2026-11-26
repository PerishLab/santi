mod notification;
mod privacy;

use santi_estate::{
    Attention, Call, Capability, Effect, Environment, Job, Notice, Prepared, Store, Strand,
    Transition, Turn,
};
use santi_model::{ingest, job, message, turn};

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const FIRST: &str = "2026-07-28T00:00:00.000Z";
const LATER: &str = "2026-07-28T00:01:00.000Z";
const FINISHED: &str = "2026-07-28T00:02:00.000Z";
const ACKED: &str = "2026-07-28T00:03:00.000Z";

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
    let turn = store
        .start(Turn {
            tag: "turn_test",
            strand: &strand.id,
            trigger: turn::Trigger::System,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
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
    let effect = store
        .stage(Effect {
            tag: "effect_test",
            turn: &turn.id,
            call: Some(&call.id),
            kind: "shell",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("effect");
    store
        .grant(Capability {
            digest: "capability_digest",
            expires: 1_000,
            soul: "soul_test",
            strand: &strand.id,
            turn: &turn.id,
            call: &call.id,
            effect: &effect.id,
            created: FIRST,
        })
        .await
        .expect("capability");

    let environment = Environment(std::collections::BTreeMap::from([(
        "PRIVATE_PROBE".to_string(),
        "frozen-secret-probe".to_string(),
    )]));
    let changed = Environment(std::collections::BTreeMap::new());
    let draft = Job {
        environment: Some(&environment),
        tag: "job_test",
        description: "test job",
        command: "true",
        cwd: Some("/tmp"),
        timeout: 30,
        limit: 1024,
        remind: Some(5),
        notify: false,
        digest: "request_digest",
        generation: "stamp_test",
        supervisor: "santi-stamp-test.service",
        created: FIRST,
    };
    let Prepared::New(created) = store
        .submit("capability_digest", draft, 500)
        .await
        .expect("prepare")
    else {
        panic!("first prepare must create");
    };
    assert_eq!(created.job.state, job::State::Submitting);
    assert_eq!(created.job.origin.soul, "soul_test");
    let Prepared::Existing(replayed) = store
        .submit(
            "capability_digest",
            Job {
                environment: Some(&changed),
                ..draft
            },
            2000,
        )
        .await
        .expect("replay")
    else {
        panic!("replay must reuse");
    };
    assert_eq!(replayed.job.id, "job_test");
    privacy::verify(&path, &replayed, &environment).await;
    assert!(
        store
            .submit(
                "capability_digest",
                Job {
                    environment: None,
                    digest: "conflict",
                    ..draft
                },
                500,
            )
            .await
            .is_err()
    );

    let accepted = store.claim("job_test", LATER).await.expect("accept");
    assert_eq!(accepted.job.state, job::State::Accepted);
    assert_eq!(accepted.job.accepted.as_deref(), Some(LATER));
    let repeated = store
        .claim("job_test", FINISHED)
        .await
        .expect("accept again");
    assert_eq!(repeated.job.accepted.as_deref(), Some(LATER));

    let running = store
        .transit(
            "job_test",
            Transition {
                started: Some(700),
                next: Some(FINISHED),
                ..Transition::new(job::State::Running, LATER)
            },
        )
        .await
        .expect("running");
    assert_eq!(running.job.started.as_deref(), Some(LATER));
    assert_eq!(running.started, Some(700));
    assert_eq!(running.job.next.as_deref(), Some(FINISHED));
    assert_eq!(store.live().await.expect("active").len(), 1);

    let source = ingest::Source::new("job").reference("job_test");
    let causes = vec!["runtime".to_string()];
    let notice = Notice {
        tag: "inbox_attention",
        strand: &strand.id,
        key: "job:job_test",
        revision: 1,
        digest: "attention_digest",
        content: &message::Content::text("job attention"),
        source: &source,
        causes: &causes,
        created: LATER,
    };
    let offered = store
        .attend(
            Attention {
                job: "job_test",
                base: 0,
                at: LATER,
                runtime: true,
                output: false,
                reminded: true,
                tick: 1,
                next: Some(FINISHED),
                terminal: false,
            },
            notice,
            10,
        )
        .await
        .expect("attention");
    assert!(offered.inserted);
    let attended = store.live().await.expect("attended").remove(0);
    assert_eq!(attended.attention, 1);
    assert!(attended.runtime);
    assert_eq!(attended.tick, 1);
    assert_eq!(attended.job.last.as_deref(), Some(LATER));
    assert_eq!(attended.job.next.as_deref(), Some(FINISHED));
    assert!(
        !store
            .attend(
                Attention {
                    job: "job_test",
                    base: 0,
                    at: LATER,
                    runtime: true,
                    output: false,
                    reminded: true,
                    tick: 1,
                    next: Some(FINISHED),
                    terminal: false,
                },
                notice,
                10,
            )
            .await
            .expect("attention replay")
            .inserted
    );
    assert!(
        store
            .attend(
                Attention {
                    job: "job_test",
                    base: 2,
                    at: LATER,
                    runtime: false,
                    output: true,
                    reminded: false,
                    tick: 1,
                    next: None,
                    terminal: false,
                },
                Notice {
                    revision: 3,
                    ..notice
                },
                10,
            )
            .await
            .is_err()
    );

    let succeeded = store
        .transit(
            "job_test",
            Transition {
                reason: Some("completed"),
                exit: Some(0),
                ..Transition::new(job::State::Succeeded, FINISHED)
            },
        )
        .await
        .expect("succeed");
    assert_eq!(succeeded.job.finished.as_deref(), Some(FINISHED));
    assert_eq!(succeeded.job.next, None);
    assert!(store.live().await.expect("inactive").is_empty());
    let other = store.job("other_soul", "job_test").await.expect("other");
    assert!(other.is_none());
    assert_eq!(store.jobs("soul_test").await.expect("jobs").len(), 1);

    let acknowledged = store.ack("job_test", ACKED).await.expect("ack");
    assert_eq!(acknowledged.job.acknowledged.as_deref(), Some(ACKED));
    store
        .ack("job_test", "2026-07-28T00:04:00.000Z")
        .await
        .expect("ack twice");
    assert_eq!(
        store.stale(ACKED, 10).await.expect("expired"),
        vec![santi_estate::Expired {
            id: "job_test".to_string(),
            key: "stamp_test".to_string(),
        }]
    );
    assert!(store.retained("stamp_test").await.expect("retained"));
    let early = store.purge("job_test", FINISHED).await.expect("too early");
    assert!(!early);
    assert!(store.purge("job_test", ACKED).await.expect("purge"));
    assert!(!store.retained("stamp_test").await.expect("gone"));
    assert!(store.submit("capability_digest", draft, 500).await.is_err());

    privacy::expire(&store, [&strand.id, &turn.id, &call.id, &effect.id]).await;

    drop(store);
    Store::open(path).await.expect("open again");
}
