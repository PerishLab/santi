use std::sync::Arc;

use async_trait::async_trait;
use futures_util::stream;
use santi_core::service::{self, Service};
use santi_core::{Store, turn};
use santi_estate::{StrandDraft, TurnDraft};
use santi_provider::{Event, Metadata, Provider, Request, Streaming};

use crate::ApiError;

struct DriverProvider;

#[async_trait]
impl Provider for DriverProvider {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("test-provider"),
            model: "test-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        Ok(Box::pin(stream::iter(vec![Ok(Event::Completed {
            response: None,
        })])))
    }
}

pub(super) async fn open(temp: &tempfile::TempDir) -> (Store, Service) {
    let database = temp.path().join("santi.sqlite");
    let store = Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
    store
        .seed(santi_core::GENESIS, &santi_core::now())
        .await
        .expect("seed");
    let service = Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
    )
    .await
    .expect("service");
    (store, service)
}

pub(super) fn ok<T>(result: Result<T, ApiError>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {}: {}", error.code(), error.message()),
    }
}

pub(super) async fn strand(store: &Store) -> String {
    store
        .create_strand(StrandDraft {
            tag: "ss_turn_read",
            soul: santi_core::GENESIS,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: &santi_core::now(),
        })
        .await
        .expect("strand")
        .id
}

pub(super) async fn turn(store: &Store, strand: &str, tag: &str) -> turn::Turn {
    store
        .create_turn(TurnDraft {
            tag,
            strand,
            trigger: turn::Trigger::StrandSend,
            from: 0,
            created: &santi_core::now(),
        })
        .await
        .expect("turn")
}
