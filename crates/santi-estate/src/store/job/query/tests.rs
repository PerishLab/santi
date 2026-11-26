use super::*;

fn invalid(query: &str) -> String {
    match native(query) {
        Err(job::Fault::Invalid(message)) => message,
        other => panic!("expected invalid query for {query:?}, got {other:?}"),
    }
}

#[test]
fn bound() {
    assert!(invalid("from Job").contains("limit or count"));
    assert!(invalid("from Job limit 0").contains("positive"));
    assert!(invalid("from Job limit 201").contains("between 1 and 200"));
    let both = invalid("from Job count limit 5");
    assert!(both.contains("count") || both.contains("limit"));
}

#[test]
fn rows() {
    assert!(native("from Job limit 5").is_ok());
    assert!(native("from Job count").is_ok());
    assert!(native("from job limit 5").is_ok());
}

#[test]
fn values() {
    assert!(native("from Job where state like \"run\" limit 5").is_ok());
}

#[test]
fn fields() {
    assert!(invalid("from Job where notify_on_terminal = \"x\" limit 5").contains("not queryable"));
    assert!(invalid("from Job where supervisor = \"x\" limit 5").contains("not queryable"));
}

#[test]
fn only() {
    assert!(invalid("from Strand limit 5").contains("from Job"));
}

use crate::{Call, Capability, Effect, Job, Prepared, Strand, Turn};
use santi_model::turn;

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NOW: &str = "2026-11-09T00:00:00.000Z";

fn count(strand: Option<&str>, all: bool) -> job::Request {
    job::Request {
        q: None,
        strand: strand.map(str::to_string),
        all,
        state: Vec::new(),
        limit: None,
        count: true,
    }
}

async fn fixture() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("santi.sqlite"), SUDO)
        .await
        .unwrap();
    for soul in ["soul-a", "soul-b"] {
        store.seed(soul, NOW).await.unwrap();
    }
    for (soul, tag) in [("soul-a", "a1"), ("soul-a", "a2"), ("soul-b", "b1")] {
        let strand = store
            .weave(Strand {
                tag,
                soul,
                label: None,
                parent: None,
                fork: None,
                inference: None,
                created: NOW,
            })
            .await
            .unwrap();
        let turn = store
            .start(Turn {
                tag: &format!("{tag}-turn"),
                strand: &strand.id,
                trigger: turn::Trigger::System,
                from: 0,
                created: NOW,
            })
            .await
            .unwrap();
        let call = store
            .calling(Call {
                tag: &format!("{tag}-call"),
                turn: &turn.id,
                tool: "shell",
                arguments: &serde_json::json!({ "command": "true" }),
                created: NOW,
            })
            .await
            .unwrap();
        let effect = store
            .stage(Effect {
                tag: &format!("{tag}-effect"),
                turn: &turn.id,
                call: Some(&call.id),
                kind: "shell",
                metadata: None,
                created: NOW,
            })
            .await
            .unwrap();
        let capability = format!("{tag}-cap");
        store
            .grant(Capability {
                digest: &capability,
                expires: 1_000,
                soul,
                strand: &strand.id,
                turn: &turn.id,
                call: &call.id,
                effect: &effect.id,
                created: NOW,
            })
            .await
            .unwrap();
        let prepared = store
            .submit(
                &capability,
                Job {
                    environment: None,
                    tag: &format!("{tag}-job"),
                    description: "test job",
                    command: "true",
                    cwd: None,
                    timeout: 30,
                    limit: 1024,
                    remind: None,
                    notify: false,
                    digest: &format!("{tag}-request"),
                    generation: &format!("{tag}-generation"),
                    supervisor: &format!("santi-{tag}.service"),
                    created: NOW,
                },
                1_000,
            )
            .await
            .unwrap();
        assert!(matches!(prepared, Prepared::New(_)));
    }
    (temp, store)
}

#[tokio::test]
async fn scope() {
    let (_temp, store) = fixture().await;
    let all = store.query("soul-a", count(None, true)).await.unwrap();
    assert!(matches!(all, job::Query::Count(job::Count { count: 2 })));

    let one = store
        .query("soul-a", count(Some("a1"), false))
        .await
        .unwrap();
    assert!(matches!(one, job::Query::Count(job::Count { count: 1 })));

    let other = store.query("soul-b", count(None, true)).await.unwrap();
    assert!(matches!(other, job::Query::Count(job::Count { count: 1 })));

    let filtered = store
        .query(
            "soul-a",
            job::Request {
                q: None,
                strand: None,
                all: true,
                state: vec!["submitting".to_string()],
                limit: None,
                count: true,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        filtered,
        job::Query::Count(job::Count { count: 2 })
    ));
}

#[tokio::test]
async fn decoded() {
    let (_temp, store) = fixture().await;
    let rows = store
        .query(
            "soul-a",
            job::Request {
                q: None,
                strand: None,
                all: true,
                state: Vec::new(),
                limit: Some(20),
                count: false,
            },
        )
        .await
        .unwrap();
    let job::Query::Rows(job::Rows { rows }) = rows else {
        panic!("expected rows");
    };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|job| job.origin.soul == "soul-a"));
}

#[tokio::test]
async fn empty() {
    let (_temp, store) = fixture().await;
    let rows = store
        .query(
            "soul-empty",
            job::Request {
                q: None,
                strand: None,
                all: true,
                state: Vec::new(),
                limit: Some(20),
                count: false,
            },
        )
        .await
        .unwrap();
    assert!(matches!(rows, job::Query::Rows(job::Rows { rows }) if rows.is_empty()));

    let count = store.query("soul-empty", count(None, true)).await.unwrap();
    assert!(matches!(count, job::Query::Count(job::Count { count: 0 })));
}

#[tokio::test]
async fn belongs() {
    let (_temp, store) = fixture().await;
    let error = store
        .query("soul-a", count(Some("b1"), false))
        .await
        .unwrap_err();
    assert!(matches!(error, job::Fault::Invalid(_)));
}
