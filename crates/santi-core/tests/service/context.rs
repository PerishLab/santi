use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{Store, message};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
struct Bounded {
    inner: FakeProvider,
    bytes: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for Bounded {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("context-provider"),
            model: "context-model".to_string(),
            budget: Some(santi_provider::Cap {
                bytes: self.bytes.load(Ordering::SeqCst),
                source: "test".to_string(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        self.inner.stream(request).await
    }
}

#[tokio::test]
async fn reuse() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let bytes = Arc::new(AtomicUsize::new(1));
    let provider = Arc::new(Bounded {
        inner: FakeProvider::default(),
        bytes: bytes.clone(),
    });
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let service = Service::open(config.clone(), provider.clone())
        .await
        .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    enqueue(&config, &strand.id, "first pending input").await;

    service.resume().await.expect("first recovery");
    assert_eq!(occurrences(&service, &strand.id).await, 1);
    service.resume().await.expect("unchanged recovery");
    assert_eq!(occurrences(&service, &strand.id).await, 1);

    drop(service);
    enqueue(&config, &strand.id, "second pending input").await;
    let service = Service::open(config, provider)
        .await
        .expect("reopen service");
    service.resume().await.expect("changed recovery");
    assert_eq!(occurrences(&service, &strand.id).await, 2);
    service
        .drive(&strand.id)
        .await
        .expect_err("explicit drive held");
    assert_eq!(occurrences(&service, &strand.id).await, 3);

    bytes.store(1_000_000, Ordering::SeqCst);
    service.resume().await.expect("expanded recovery");
    Probe::new(&service).any_completed(&strand.id).await;
    let incidents = service
        .stranded(&strand.id, 10)
        .await
        .expect("read incidents")
        .expect("strand exists");
    let incident = incidents
        .iter()
        .find(|incident| incident.code == "context.budget.exceeded")
        .expect("context incident");
    assert_eq!(incident.status, santi_core::Status::Resolved);
    assert_eq!(
        incident
            .resolution
            .as_ref()
            .and_then(|value| value.by.as_deref()),
        Some("driver_remeasurement")
    );
}

async fn enqueue(config: &service::Config, strand: &str, text: &str) {
    let store = Store::open(&config.database).await.expect("open store");
    let content = message::Content::text(text);
    store
        .accept_inbox(
            santi_core::InboxDraft {
                tag: &santi_core::tag("inbox"),
                strand,
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .expect("enqueue pending input");
}

async fn occurrences(service: &Service, strand: &str) -> i64 {
    let snapshot = service
        .audit(strand)
        .await
        .expect("read runtime")
        .expect("runtime exists");
    let incident = snapshot.incident.expect("context incident");
    assert_eq!(incident.code, "context.budget.exceeded");
    incident.occurrences
}
