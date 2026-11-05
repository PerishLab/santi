use super::*;
use async_trait::async_trait;
use santi_provider::{Metadata, Provider, Request, Streaming};
use std::sync::Arc;

struct Capped;

#[async_trait]
impl Provider for Capped {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("capped"),
            model: "capped".into(),
            budget: Some(santi_provider::Cap {
                bytes: 100,
                maintenance: 200,
                source: "test".into(),
            }),
        }
    }

    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        Err("audit test does not stream".into())
    }
}

async fn opened() -> (tempfile::TempDir, Service, String) {
    let temp = tempfile::tempdir().unwrap();
    crate::Store::bootstrap(
        temp.path().join("santi.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let config = crate::service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let service = Service::open(config, Arc::new(Capped)).await.unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    (temp, service, strand)
}

#[tokio::test]
async fn probe() {
    let (temp, service, strand) = opened().await;
    let provider = service.preview(&strand).await.unwrap();
    let composition = service.composition(&strand).await.unwrap();
    let empty = service
        .weighed(provider.as_ref(), &strand, composition.text.clone())
        .await
        .unwrap();
    let ordinary = service
        .weight(provider.as_ref(), &strand, false)
        .await
        .unwrap();
    let prior = service.gauge(&strand, true).await.unwrap();
    assert_ne!(empty.tools, ordinary.tools);
    let audit = service.audit(&strand).await.unwrap().unwrap();
    assert_eq!(audit.request.estimate, empty);
    let pressure = audit.pressure.unwrap();
    assert_eq!(pressure.active, prior.active);
    assert_eq!(pressure.ceiling, prior.ceiling.unwrap());

    let content = crate::message::Content::text("pending bytes that must enter both projections");
    crate::Store::open(&temp.path().join("santi.sqlite").display().to_string())
        .await
        .unwrap()
        .accept_inbox(
            crate::InboxDraft {
                tag: &crate::tag("inbox"),
                strand: &strand,
                kind: crate::message::Kind::Text,
                content: &content,
                source: None,
                created: &crate::now(),
            },
            500,
        )
        .await
        .unwrap();

    let pending = service
        .weighed(provider.as_ref(), &strand, composition.text)
        .await
        .unwrap();
    let ordinary = service
        .weight(provider.as_ref(), &strand, true)
        .await
        .unwrap();
    let gauge = service.gauge(&strand, true).await.unwrap();
    assert_ne!(pending.tools, ordinary.tools);
    let audit = service.audit(&strand).await.unwrap().unwrap();
    assert_eq!(audit.request.estimate, pending);
    assert_eq!(audit.request.estimate.tools, pending.tools);
    let pressure = audit.pressure.unwrap();
    assert_eq!(pressure.active, gauge.active);
    assert_eq!(pressure.ceiling, gauge.ceiling.unwrap());
    assert!(audit.request.estimate.input > empty.input);
    assert!(pressure.active > prior.active);
}
