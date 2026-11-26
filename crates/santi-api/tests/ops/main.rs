use std::path::{Path, PathBuf};

use santi_api::config::Layout;

fn layout(root: &Path) -> Layout {
    Layout {
        database: root.join("runtime").join("db"),
        runtime: root.join("runtime"),
        execution: root.join("execution"),
    }
}

mod runtime {
    use super::*;

    #[tokio::test]
    async fn reads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");
        let memory = paths
            .runtime
            .join("souls")
            .join(santi_core::GENESIS)
            .join("memory")
            .join(santi_core::MEMORY);
        std::fs::create_dir_all(memory.parent().unwrap()).unwrap();
        std::fs::write(&memory, "# memory").unwrap();

        let report = paths.doctor().await.expect("doctor");
        assert!(report.ok, "expected healthy: {report:?}");
        assert!(report.bound);
        assert!(report.ready);
        assert!(report.error.is_none());
        assert!(report.present && report.readable);
        assert!(report.bytes > 0);
    }

    #[tokio::test]
    async fn rejects() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        std::fs::create_dir_all(paths.database.parent().unwrap()).unwrap();
        std::fs::write(&paths.database, b"not an estate").expect("write malformed estate");

        let report = paths.doctor().await.expect("doctor");
        assert!(!report.ok);
        assert!(!report.bound);
        assert!(!report.ready);
        assert!(report.error.is_some());
    }

    #[tokio::test]
    async fn handles() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");
        let report = paths.doctor().await.expect("doctor");
        assert!(report.ok, "absent memory should be fine: {report:?}");
        assert!(!report.present);

        let missing = Layout {
            database: temp.path().join("void").join("db"),
            ..paths
        };
        let report = missing.doctor().await.expect("doctor");
        assert!(!report.ok);
        assert!(!report.exists);
        assert!(!report.bound);
        assert_eq!(report.error.as_deref(), Some("estate database is missing"));
    }

    #[tokio::test]
    async fn binds() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        drop(super::support::bootstrap(&paths.database).await);

        let report = paths.doctor().await.expect("doctor");
        assert!(report.bound);
        assert!(!report.ready);
        assert_eq!(report.error.as_deref(), Some("genesis soul is missing"));
    }

    #[tokio::test]
    async fn serializes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");
        let report = paths.doctor().await.expect("doctor");
        let json = serde_json::to_string(&report).expect("serialize");
        assert!(json.contains("\"estate_bound\""));
        assert!(json.contains("\"estate_ready\""));
        assert!(json.contains("\"provider\":null"));
        let _ = PathBuf::from(&report.database);
    }
}

mod seed {
    use super::*;

    #[tokio::test]
    async fn boots() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let strand = {
            let store = super::support::bootstrap(&paths.database).await;
            store
                .seed(santi_core::GENESIS, &santi_core::now())
                .await
                .expect("seed");
            store
                .weave(santi_estate::Strand {
                    tag: "ss_seed",
                    soul: santi_core::GENESIS,
                    label: None,
                    parent: None,
                    fork: None,
                    inference: None,
                    created: &santi_core::now(),
                })
                .await
                .expect("create strand")
                .id
        };

        let report = paths.seed(&strand, "come look").await.unwrap();
        assert!(report.accepted);
        let store = santi_core::Store::open(&paths.database)
            .await
            .expect("reopen");
        let inboxes = store.inboxes(&strand).await.expect("pending inbox");
        assert_eq!(inboxes.len(), 1);
        assert_eq!(inboxes[0].content.rendered(), "come look");
        assert_eq!(inboxes[0].kind, santi_core::message::Kind::System);
    }

    #[tokio::test]
    async fn labels() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");
        let label = "soul:soul_default:ops";

        let report = paths
            .labeled(santi_core::GENESIS, label, "upgrade finished")
            .await
            .unwrap();
        assert!(report.accepted);
        let store = santi_core::Store::open(&paths.database)
            .await
            .expect("reopen");
        let strand = store.strand(&report.strand).await.unwrap().expect("strand");
        assert_eq!(strand.label.as_deref(), Some(label));
        let inboxes = store.inboxes(&report.strand).await.expect("inboxes");
        assert_eq!(inboxes[0].content.rendered(), "upgrade finished");
    }

    #[tokio::test]
    async fn unknown() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");

        let error = paths.seed("ss_missing", "x").await.unwrap_err();
        assert!(error.contains("unknown strand"), "got: {error}");
        let store = santi_core::Store::open(&paths.database)
            .await
            .expect("reopen");
        assert!(store.waiting().await.unwrap().is_empty());
    }
}

mod audit {
    use super::*;

    #[tokio::test]
    async fn projects() {
        let temp = tempfile::tempdir().expect("temp dir");
        let paths = layout(temp.path());
        let store = super::support::bootstrap(&paths.database).await;
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .expect("seed");
        store
            .weave(santi_estate::Strand {
                tag: "ss_audit",
                soul: santi_core::GENESIS,
                label: None,
                parent: None,
                fork: None,
                inference: None,
                created: "2026-07-28T00:00:00.000Z",
            })
            .await
            .expect("strand");
        store
            .start(santi_estate::Turn {
                tag: "turn_audit",
                strand: "ss_audit",
                trigger: santi_core::turn::Trigger::System,
                from: 0,
                created: "2026-07-28T00:00:00.000Z",
            })
            .await
            .expect("turn");
        store
            .calling(santi_estate::Call {
                tag: "call_audit",
                turn: "turn_audit",
                tool: "shell",
                arguments: &serde_json::json!({"command": "printf audit"}),
                created: "2026-07-28T00:00:01.000Z",
            })
            .await
            .expect("call");
        store
            .replying(santi_estate::Reply {
                tag: "result_audit",
                call: "call_audit",
                output: Some(&serde_json::json!({"exit_code": 0, "stdout": "audit"})),
                error: None,
                created: "2026-07-28T00:00:02.000Z",
            })
            .await
            .expect("reply");
        store
            .complete("turn_audit", 2, "2026-07-28T00:00:03.000Z")
            .await
            .expect("complete");

        let rows = paths.audit(query(false)).await.expect("audit");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].turn, "turn_audit");
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].arguments["command"], "printf audit");
        assert_eq!(rows[0].output.as_ref().unwrap()["stdout"], "audit");
        assert!(paths.audit(query(true)).await.expect("failed").is_empty());
    }
}

fn query(failed: bool) -> santi_api::ops::Audit<'static> {
    santi_api::ops::Audit {
        strand: Some("ss_audit"),
        turn: None,
        failed,
        limit: 30,
        after: None,
    }
}

mod doctor;
mod support;
