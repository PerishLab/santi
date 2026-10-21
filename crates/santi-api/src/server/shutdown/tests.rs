use super::*;
use async_trait::async_trait;
use santi_core::{message, service, strand, turn};
use santi_provider::{Metadata, Provider, Request, Streaming};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, oneshot};

#[derive(Default)]
struct Waiting {
    entered: Notify,
}

#[async_trait]
impl Provider for Waiting {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("shutdown"),
            model: "shutdown".into(),
            budget: None,
        }
    }

    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        self.entered.notify_one();
        std::future::pending().await
    }
}

async fn open(temp: &tempfile::TempDir, provider: Arc<Waiting>) -> Service {
    santi_core::Store::bootstrap(
        temp.path().join("db"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    Service::open(
        service::Config {
            database: temp.path().join("db").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn expires() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(&temp, Arc::new(Waiting::default())).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (signal, received) = oneshot::channel();
    let task = tokio::spawn(serve(
        service.clone(),
        listener,
        Duration::from_millis(50),
        async {
            received.await.unwrap();
        },
    ));
    let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
    client.write_all(b"POST /api/v1/strands/ss_probe/send HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{")
        .await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    signal.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(service.closing());
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    drop(client);
    let snapshot = service.weave().await.unwrap();
    let runtime = service
        .snapshot(&snapshot.strand.id)
        .await
        .unwrap()
        .unwrap();
    assert!(runtime.turns.is_empty());
    assert!(runtime.effects.is_empty());
}

#[tokio::test]
async fn settles() {
    let temp = tempfile::tempdir().unwrap();
    let provider = Arc::new(Waiting::default());
    let service = open(&temp, provider.clone()).await;
    let strand = service.weave().await.unwrap().strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "pending provider work".into(),
                }],
            },
        )
        .await
        .unwrap();
    let prior = service
        .receipt(&posted.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        serve(
            service.clone(),
            listener,
            Duration::from_millis(20),
            async {},
        ),
    )
    .await;
    if result.is_err() {
        let runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
        panic!("shutdown timed out: {:?}", runtime.turns);
    }
    result.unwrap().unwrap();
    let turn = service
        .receipt(&posted.receipt.inbox)
        .await
        .unwrap()
        .unwrap()
        .transitions
        .iter()
        .find_map(|transition| transition.turn.clone())
        .expect("shutdown receipt identifies its turn");
    let runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
    let held = runtime.turns.iter().find(|held| held.id == turn).unwrap();
    assert_eq!(held.status, turn::Status::Failed);
    assert_eq!(held.error.as_deref(), Some("interrupted by shutdown"));
    let receipt = service
        .receipt(&posted.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.accepted, prior.accepted);
    assert_eq!(receipt.state, santi_core::receipt::State::Failed);
    let store = santi_core::Store::open(temp.path().join("db"))
        .await
        .unwrap();
    let stop = store.stop(&turn).await.unwrap().unwrap();
    assert_eq!(stop.cause, Some(turn::Cause::Shutdown));
    assert!(stop.settled.is_some());
    assert!(runtime.effects.is_empty());
}
