use crate::{
    environ::Scope,
    material,
    service::{Config, Service},
};
use radicula_lock::{Error, Wait};
use santi_provider::{Metadata, Provider, Request, Streaming};
use std::{sync::Arc, time::Duration};

struct Stub;

#[async_trait::async_trait]
impl Provider for Stub {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("fixture"),
            model: "fixture".into(),
            budget: None,
        }
    }

    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        Err("fixture has no provider execution".into())
    }
}

struct Fixture {
    service: Service,
    _temp: tempfile::TempDir,
}

impl Fixture {
    async fn open() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("santi.sqlite");
        crate::Store::bootstrap(
            &database,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .await
        .unwrap();
        let service = Service::open(
            Config {
                database: database.display().to_string(),
                runtime: temp.path().join("runtime").display().to_string(),
                execution: temp.path().join("execution").display().to_string(),
                bind: None,
                constitution: None,
                environment: Default::default(),
            },
            Arc::new(Stub),
        )
        .await
        .unwrap();
        Self {
            service,
            _temp: temp,
        }
    }
}

fn command() -> material::Command {
    material::Command {
        reference: "absent".into(),
        operation: None,
    }
}

#[tokio::test]
async fn binding() {
    let fixture = Fixture::open().await;
    let service = fixture.service.clone();
    let held = fixture
        .service
        .binding
        .tokio()
        .acquire(Wait::immediate())
        .await
        .unwrap();
    let mut loading = Box::pin(service.load(Scope::Strand, "absent", command()));
    assert!(futures_util::poll!(loading.as_mut()).is_pending());
    drop(loading);
    assert_eq!(service.binding.status().active, 1);
    held.release().await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_secs(2),
            service.load(Scope::Strand, "absent", command())
        )
        .await
        .unwrap()
        .is_err()
    );
    assert_eq!(service.binding.status().active, 0);
    assert!(
        tokio::time::timeout(
            Duration::from_secs(2),
            service.drop(Scope::Strand, "absent", command())
        )
        .await
        .unwrap()
        .is_err()
    );
    assert_eq!(service.binding.status().active, 0);
}

#[tokio::test]
async fn pressure() {
    let fixture = Fixture::open().await;
    let service = fixture.service.clone();
    let strand = service.weave().await.unwrap().strand;
    let held = fixture
        .service
        .pressure
        .tokio()
        .acquire(Wait::immediate())
        .await
        .unwrap();
    let mut waiting = Box::pin(service.gate(&strand));
    assert!(futures_util::poll!(waiting.as_mut()).is_pending());
    drop(waiting);
    assert_eq!(service.pressure.status().active, 1);
    held.release().await.unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), service.gate(&strand))
            .await
            .unwrap()
            .unwrap(),
        super::Gate::Allow
    ));
    assert_eq!(service.pressure.status().active, 0);
}

#[tokio::test]
async fn domains() {
    let first = Fixture::open().await;
    let second = Fixture::open().await;
    let binding = first
        .service
        .binding
        .tokio()
        .acquire(Wait::immediate())
        .await
        .unwrap();
    let pressure = first
        .service
        .pressure
        .tokio()
        .acquire(Wait::immediate())
        .await
        .unwrap();
    assert!(matches!(
        first.service.clone().binding.acquire(Wait::immediate()),
        Err(Error::Busy)
    ));
    assert!(
        tokio::time::timeout(
            Duration::from_secs(2),
            second.service.load(Scope::Strand, "absent", command())
        )
        .await
        .unwrap()
        .is_err()
    );
    let strand = second.service.weave().await.unwrap().strand;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), second.service.gate(&strand))
            .await
            .unwrap()
            .unwrap(),
        super::Gate::Allow
    ));
    assert_eq!(first.service.binding.status().active, 1);
    assert_eq!(first.service.pressure.status().active, 1);
    binding.release().await.unwrap();
    pressure.release().await.unwrap();
}
