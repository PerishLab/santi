use super::super::*;
use async_trait::async_trait;
use futures_util::stream;
use santi_core::{Store, message, strand};
use santi_estate::{
    AttentionDraft, CallDraft, CapabilityDraft, EffectDraft, JobDraft, NoticeDraft,
    TransitionDraft, TurnDraft,
};
use santi_provider::{Event, Metadata, Provider, Request, Streaming};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

pub(super) fn config(temp: &tempfile::TempDir) -> service::Config {
    service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: Some("127.0.0.1:0".to_string()),
        constitution: None,
        environment: Default::default(),
    }
}

pub(super) async fn reopen(
    temp: &tempfile::TempDir,
    provider: Arc<dyn Provider>,
    supervisor: Arc<dyn JobSupervisor>,
) -> (Service, Store) {
    let service = Service::supervised(config(temp), provider, supervisor)
        .await
        .expect("reopen service");
    let store = Store::open(config(temp).database)
        .await
        .expect("reopen estate");
    (service, store)
}

pub(super) async fn boot(
    temp: &tempfile::TempDir,
    provider: Arc<dyn Provider>,
    supervisor: Arc<dyn JobSupervisor>,
) -> (Service, Store, strand::Strand) {
    crate::support::bootstrap(&temp.path().join("santi.sqlite")).await;
    let service = Service::supervised(config(temp), provider, supervisor)
        .await
        .expect("open service");
    let store = Store::open(config(temp).database)
        .await
        .expect("open estate");
    let strand = service.weave().await.expect("create strand").strand;
    (service, store, strand)
}

pub(super) struct Terminal<'a> {
    pub(super) store: &'a Store,
    pub(super) strand: &'a strand::Strand,
    pub(super) job: &'a str,
    pub(super) notice: &'a str,
    pub(super) text: &'a str,
}

pub(super) async fn terminal(case: Terminal<'_>) -> String {
    let Terminal {
        store,
        strand,
        job,
        notice,
        text,
    } = case;
    let digest = format!("{:x}", Sha256::digest(fixture::CAPABILITY.as_bytes()));
    let now = santi_core::now();
    let created = store
        .prepare_job(
            &digest,
            JobDraft {
                environment: None,
                tag: job,
                description: "durable terminal notice",
                command: "true",
                cwd: None,
                timeout_seconds: 30,
                output_limit_bytes: 1024,
                remind_every_seconds: None,
                notify_on_terminal: true,
                request_sha256: "request_terminal",
                generation: "stamp_terminal",
                supervisor_ref: "santi-term.service",
                created: &now,
            },
            0,
        )
        .await
        .expect("prepare");
    let santi_estate::Prepared::New(record) = created else {
        panic!("new job");
    };
    store
        .accept_job(&record.job.id, &now)
        .await
        .expect("accept");
    store
        .transition_job(
            &record.job.id,
            TransitionDraft {
                reason: Some("completed"),
                exit_code: Some(0),
                ..TransitionDraft::new(job::State::Succeeded, &now)
            },
        )
        .await
        .expect("terminal");
    let source = santi_core::ingest::Source::new("job").with_ref(job);
    let causes = vec!["terminal".to_string()];
    let content = message::Content::text(text);
    let key = format!("job/{job}/stamp_terminal");
    store
        .attend_job(
            AttentionDraft {
                job,
                base: 0,
                at: &now,
                runtime: false,
                output: false,
                reminded: false,
                tick: 0,
                next: None,
                terminal: true,
            },
            NoticeDraft {
                tag: notice,
                strand: &strand.id,
                key: &key,
                revision: 1,
                digest: "terminal_digest",
                content: &content,
                source: &source,
                causes: &causes,
                created: &now,
            },
            10,
        )
        .await
        .expect("durable notice");
    record.job.id
}

pub(super) async fn origin(store: &Store, strand: &strand::Strand, turn: &str) {
    let now = santi_core::now();
    let call = format!("{turn}_call");
    let effect = format!("{turn}_effect");
    store
        .create_turn(TurnDraft {
            tag: turn,
            strand: &strand.id,
            trigger: santi_core::turn::Trigger::System,
            from: 0,
            created: &now,
        })
        .await
        .expect("origin turn");
    store
        .create_call(CallDraft {
            tag: &call,
            turn,
            tool: "shell",
            arguments: &serde_json::json!({"command": "printf ok"}),
            created: &now,
        })
        .await
        .expect("origin call");
    store
        .prepare_effect(EffectDraft {
            tag: &effect,
            turn,
            call: Some(&call),
            kind: "shell",
            metadata: None,
            created: &now,
        })
        .await
        .expect("origin effect");
    let digest = format!("{:x}", Sha256::digest(fixture::CAPABILITY.as_bytes()));
    store
        .create_capability(CapabilityDraft {
            digest: &digest,
            expires: i64::MAX,
            soul: &strand.soul,
            strand: &strand.id,
            turn,
            call: &call,
            effect: &effect,
            created: &now,
        })
        .await
        .expect("origin capability");
    let done = store
        .complete_turn(turn, 1, &now)
        .await
        .expect("finish origin turn");
    assert_eq!(done.status, santi_core::turn::Status::Completed);
}

#[derive(Default)]
pub(super) struct Recording {
    pub(super) requests: Mutex<Vec<Request>>,
}

#[async_trait]
impl Provider for Recording {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("recording-provider"),
            model: "recording-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        self.requests.lock().unwrap().push(request);
        Ok(Box::pin(stream::iter(vec![Ok(Event::Completed {
            response: None,
        })])))
    }
}

pub(super) struct Gated {
    pub(super) requests: Mutex<Vec<Request>>,
    pub(super) entered: Notify,
    pub(super) release: Notify,
}

impl Gated {
    pub(super) fn new() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}

#[async_trait]
impl Provider for Gated {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("gated-provider"),
            model: "gated-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let index = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len() - 1
        };
        if index == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(Box::pin(stream::iter(vec![Ok(Event::Completed {
            response: None,
        })])))
    }
}
