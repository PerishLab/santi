pub(crate) use async_trait::async_trait;
pub(crate) use futures_util::stream;
use santi_core::service::Service;
use santi_core::strand;
pub(crate) use santi_core::{SOULSPACE, STRANDSPACE, soulward, strandward};

mod probe;
pub(crate) use probe::*;

pub(crate) async fn accepted(
    service: &Service,
    response: &strand::Posted,
) -> santi_core::turn::Turn {
    for _ in 0..100 {
        let receipt = service
            .receipt(&response.receipt.inbox)
            .await
            .expect("receipt")
            .expect("accepted");
        if let Some(turn) = receipt
            .transitions
            .iter()
            .find_map(|transition| transition.turn.as_ref())
        {
            return service
                .snapshot(&response.strand.id)
                .await
                .expect("snapshot")
                .expect("strand")
                .turns
                .into_iter()
                .find(|held| &held.id == turn)
                .expect("receipt turn");
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("accepted receipt did not start processing");
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
