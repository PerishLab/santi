pub(crate) use async_trait::async_trait;
pub(crate) use futures_util::stream;
use santi_core::service::Service;
use santi_core::strand;
pub(crate) use santi_core::{SOULSPACE, STRANDSPACE, soulward, strandward};

use tokio::sync::broadcast::error::RecvError;

mod probe;
pub(crate) use probe::*;

pub(crate) const WAIT: Duration = Duration::from_secs(10);

pub(crate) async fn eventually<T>(
    service: &Service,
    what: &str,
    mut probe: impl AsyncFnMut() -> Option<T>,
) -> T {
    let mut events = service.listen().await;
    let wait = async {
        loop {
            if let Some(found) = probe().await {
                return found;
            }
            if matches!(events.recv().await, Err(RecvError::Closed)) {
                panic!("service stream closed while waiting for {what}")
            }
        }
    };
    tokio::time::timeout(WAIT, wait)
        .await
        .unwrap_or_else(|_| panic!("timed out after {WAIT:?} waiting for {what}"))
}

pub(crate) async fn accepted(
    service: &Service,
    response: &strand::Posted,
) -> santi_core::turn::Turn {
    let turn = eventually(
        service,
        "accepted receipt to start processing",
        async || {
            let receipt = service
                .receipt(&response.receipt.inbox)
                .await
                .expect("receipt")
                .expect("accepted");
            receipt
                .transitions
                .iter()
                .find_map(|transition| transition.turn.clone())
        },
    )
    .await;
    service
        .snapshot(&response.strand.id)
        .await
        .expect("snapshot")
        .expect("strand")
        .turns
        .into_iter()
        .find(|held| held.id == turn)
        .expect("receipt turn")
}
pub(crate) use santi_provider::{Event, Item, Metadata, Provider, Request, Streaming};
pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use tokio::time::{Duration, sleep};

pub(crate) async fn bootstrap(temp: &tempfile::TempDir) {
    santi_core::Store::bootstrap(
        temp.path().join("santi.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
}

#[derive(Clone, Default)]
pub(crate) struct Fake {
    pub(crate) requests: Arc<Mutex<Vec<Request>>>,
}

#[async_trait]
impl Provider for Fake {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("fake-provider"),
            model: "fake-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("hi from runtime".to_string())),
            Ok(Event::Completed {
                response: Some("fake-response-id".to_string()),
            }),
        ])))
    }
}

pub(crate) fn fixed(scenario: &str, model: &str) -> santi_core::inference::Effective {
    santi_core::inference::Effective {
        selection: santi_core::inference::Selection::Scenario(scenario.into()),
        model: model.to_string(),
        effort: None,
        source: santi_core::inference::Source {
            route: None,
            selection: santi_core::inference::Selection::Scenario(
                santi_core::inference::Origin::Service,
            ),
            model: santi_core::inference::Origin::Default,
            effort: None,
        },
    }
}

pub(crate) struct Seed<'a> {
    pub(crate) tag: &'a str,
    pub(crate) text: &'a str,
    pub(crate) effective: &'a santi_core::inference::Effective,
}

pub(crate) async fn pinned(
    store: &santi_core::Store,
    strand: &str,
    seed: Seed<'_>,
) -> santi_core::turn::Turn {
    let Seed {
        tag,
        text,
        effective,
    } = seed;
    let content = santi_core::message::Content::text(text);
    store
        .accept(
            santi_core::Arrival {
                tag: &santi_core::tag("inbox"),
                strand,
                kind: santi_core::message::Kind::Text,
                content: &content,
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .expect("enqueue fixture input");
    match store
        .begin(
            santi_estate::Drain {
                turn: tag,
                strand,
                trigger: santi_core::turn::Trigger::System,
                actor: "test",
                created: &santi_core::now(),
            },
            effective,
        )
        .await
        .expect("drain fixture turn")
    {
        santi_estate::Opening::Started(begun) => begun.turn,
        other => panic!("fixture turn did not start: {other:?}"),
    }
}

pub(crate) async fn until(
    service: &Service,
    store: &santi_core::Store,
    strand: &str,
    status: santi_core::turn::Status,
) {
    eventually(
        service,
        &format!("the turn to reach {status:?}"),
        async || {
            (store.latest(strand).await.unwrap().map(|turn| turn.status) == Some(status.clone()))
                .then_some(())
        },
    )
    .await;
}
