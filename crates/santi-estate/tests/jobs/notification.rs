use super::*;

const OLD: &str = "2026-07-28T00:00:00.000Z";
const MID: &str = "2026-07-28T00:01:00.000Z";
const NEW: &str = "2026-07-28T00:02:00.000Z";

struct Probe<'a> {
    store: &'a Store,
    strand: &'a str,
}

impl Probe<'_> {
    async fn seed(&self, suffix: &str, notify: bool, created: &str) -> santi_estate::JobRecord {
        let capability = format!("capability_{suffix}");
        self.store
            .create_capability(CapabilityDraft {
                digest: &capability,
                expires: 10_000,
                soul: "soul_test",
                strand: self.strand,
                turn: "turn_test",
                call: "call_test",
                effect: "effect_test",
                created,
            })
            .await
            .expect("capability");
        let tag = format!("job_{suffix}");
        let digest = format!("request_{suffix}");
        let stamp = format!("stamp_{suffix}");
        let supervisor = format!("santi-{suffix}.service");
        let Prepared::New(record) = self
            .store
            .prepare_job(
                &capability,
                JobDraft {
                    environment: None,
                    tag: &tag,
                    description: "notify probe",
                    command: "true",
                    cwd: None,
                    timeout_seconds: 30,
                    output_limit_bytes: 1024,
                    remind_every_seconds: None,
                    notify_on_terminal: notify,
                    request_sha256: &digest,
                    generation: &stamp,
                    supervisor_ref: &supervisor,
                    created,
                },
                100,
            )
            .await
            .expect("prepare")
        else {
            panic!("new job");
        };
        assert_eq!(record.notify_on_terminal, notify, "prepared notify flag");
        self.store.accept_job(&tag, created).await.expect("accept")
    }

    async fn terminal(&self, tag: &str, at: &str) -> santi_estate::JobRecord {
        self.store
            .transition_job(
                tag,
                TransitionDraft {
                    reason: Some("completed"),
                    exit_code: Some(0),
                    ..TransitionDraft::new(job::State::Succeeded, at)
                },
            )
            .await
            .expect("terminal")
    }

    async fn notify(&self, tag: &str, at: &str, gate: usize) -> Result<(), String> {
        let causes = vec!["terminal".to_string()];
        let source = ingest::Source::new("job").with_ref(tag);
        let content = message::Content::text("terminal fact");
        let inbox = format!("inbox_{tag}");
        self.store
            .attend_job(
                AttentionDraft {
                    job: tag,
                    base: 0,
                    at,
                    runtime: false,
                    output: false,
                    reminded: false,
                    tick: 0,
                    next: None,
                    terminal: true,
                },
                NoticeDraft {
                    tag: &inbox,
                    strand: self.strand,
                    key: &format!("job/{tag}/stamp"),
                    revision: 1,
                    digest: "terminal_digest",
                    content: &content,
                    source: &source,
                    causes: &causes,
                    created: at,
                },
                gate,
            )
            .await
            .map(|_| ())
    }
}

#[tokio::test]
async fn atomic() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(&temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    store.seed("soul_test", OLD).await.expect("seed");
    let strand = store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: OLD,
        })
        .await
        .expect("strand")
        .id;
    store
        .create_turn(TurnDraft {
            tag: "turn_test",
            strand: &strand,
            trigger: turn::Trigger::System,
            from: 0,
            created: OLD,
        })
        .await
        .expect("turn");
    store
        .create_call(CallDraft {
            tag: "call_test",
            turn: "turn_test",
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: OLD,
        })
        .await
        .expect("call");
    store
        .prepare_effect(EffectDraft {
            tag: "effect_test",
            turn: "turn_test",
            call: Some("call_test"),
            kind: "shell",
            metadata: None,
            created: OLD,
        })
        .await
        .expect("effect");

    let probe = Probe {
        store: &store,
        strand: &strand,
    };
    for suffix in ["old_a", "old_b"] {
        probe.seed(suffix, true, OLD).await;
        probe.terminal(&format!("job_{suffix}"), OLD).await;
        probe
            .notify(&format!("job_{suffix}"), OLD, 10)
            .await
            .expect("old notification");
    }
    let pending = probe.seed("pending", true, MID).await;
    assert!(pending.notify_on_terminal);
    probe.terminal("job_pending", MID).await;
    assert_eq!(
        store
            .unnotified(1)
            .await
            .expect("unnotified")
            .into_iter()
            .map(|record| record.job.id)
            .collect::<Vec<_>>(),
        vec!["job_pending".to_string()]
    );

    probe
        .notify("job_pending", MID, 0)
        .await
        .expect_err("capacity must fail");
    let held = store
        .job_record("job_pending")
        .await
        .expect("record")
        .expect("job");
    assert_eq!(held.job.state, job::State::Succeeded);
    assert!(held.terminal_notified.is_none());
    assert_eq!(store.unnotified(1).await.expect("still pending").len(), 1);

    probe.notify("job_pending", NEW, 10).await.expect("repair");
    assert!(store.unnotified(1).await.expect("notified").is_empty());
    assert!(
        store
            .job_record("job_pending")
            .await
            .expect("record")
            .expect("job")
            .terminal_notified
            .is_some()
    );

    let quiet = probe.seed("quiet", false, NEW).await;
    assert!(!quiet.notify_on_terminal);
    probe.terminal("job_quiet", NEW).await;
    assert!(
        !store
            .unnotified(10)
            .await
            .expect("quiet")
            .iter()
            .any(|record| record.job.id == "job_quiet")
    );
    assert_eq!(pending.attention_revision, 0);
}
