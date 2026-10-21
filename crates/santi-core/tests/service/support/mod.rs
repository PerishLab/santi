pub(crate) use async_trait::async_trait;
pub(crate) use futures_util::stream;
use santi_core::service::Service;
use santi_core::strand;
pub(crate) use santi_core::{SOULSPACE, STRANDSPACE, soulward, strandward};

mod probe;
pub(crate) use probe::*;

pub(crate) async fn accepted_turn(
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
pub(crate) struct FakeProvider {
    pub(crate) requests: Arc<Mutex<Vec<Request>>>,
}

#[async_trait]
impl Provider for FakeProvider {
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
