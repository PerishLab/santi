use super::*;

pub(super) async fn turn(
    service: &Service,
    response: &santi_core::strand::Posted,
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

pub(super) async fn open_service(
    temp: &tempfile::TempDir,
    provider: Arc<FailureProvider>,
) -> Service {
    santi_core::Store::bootstrap(
        temp.path().join("santi.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .expect("open service")
}

pub(super) async fn send_text(
    service: &Service,
    strand: &str,
    text: &str,
) -> santi_core::strand::Posted {
    service
        .send(
            strand,
            strand::Post {
                content: vec![message::Part::Text {
                    text: text.to_string(),
                }],
            },
        )
        .await
        .expect("send strand")
}

pub(super) async fn wait_for_turn(
    service: &Service,
    strand: &str,
    turn: &str,
    status: turn::Status,
) -> santi_core::stream::Snapshot {
    for _ in 0..100 {
        let runtime = service
            .snapshot(strand)
            .await
            .expect("runtime snapshot")
            .expect("strand runtime");
        if runtime
            .turns
            .iter()
            .any(|held| held.id == turn && held.status == status)
        {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("turn {turn} did not reach {status:?}");
}
