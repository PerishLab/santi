use super::super::*;
use super::support::{Recording, Terminal, reopen, terminal};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn coldstart() {
    let temp = tempfile::tempdir().expect("temp dir");
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let (service, strand, store) = fixture::prepare(&temp, supervisor.clone()).await;
    let now = santi_core::now();
    let origin = store
        .complete_turn("turn_http", 1, &now)
        .await
        .expect("finish origin turn");
    assert_eq!(origin.status, santi_core::turn::Status::Completed);

    let job = terminal(Terminal {
        store: &store,
        strand: &strand,
        job: "job_terminal",
        notice: "inbox_terminal",
        text: "terminal fact",
    })
    .await;
    assert_eq!(store.inboxes(&strand.id).await.expect("pending").len(), 1);
    let record = store.job_record(&job).await.expect("record").expect("job");
    assert!(record.notify_on_terminal);
    assert!(record.terminal_notified.is_some());
    assert!(store.unnotified(10).await.expect("unnotified").is_empty());

    drop(store);
    drop(service);
    let provider = Arc::new(Recording::default());
    let (service, store) = reopen(&temp, provider.clone(), supervisor.clone()).await;
    service.resume().await.expect("cold start resume");

    let mut drained = false;
    for _ in 0..250 {
        if !provider.requests.lock().unwrap().is_empty() {
            drained = true;
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(drained, "resume did not consume the terminal notice");

    let mut finished = None;
    for _ in 0..250 {
        let turns = store.turns(&strand.id).await.expect("turns");
        if let Some(turn) = turns.iter().find(|turn| {
            turn.id != "turn_http" && turn.status == santi_core::turn::Status::Completed
        }) {
            finished = Some(turn.id.clone());
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let finished = finished.expect("recovered turn did not complete");
    assert_ne!(finished, "turn_http");

    {
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1, "recovery must start exactly one turn");
        let messages: Vec<&str> = requests[0]
            .input
            .iter()
            .filter_map(|item| match item {
                santi_provider::Item::Message { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            messages
                .iter()
                .any(|content| content.contains("terminal fact")),
            "provider request is missing the terminal notice: {messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|content| content.contains("causes: [\"terminal\"]")),
            "provider request is missing the terminal cause: {messages:?}"
        );
    }

    assert!(
        store.inboxes(&strand.id).await.expect("drained").is_empty(),
        "durable terminal notice remained pending"
    );
    let job = store.job_record(&job).await.expect("record").expect("job");
    assert_eq!(job.job.state, job::State::Succeeded);
    assert_eq!(job.job.id, "job_terminal");
    assert_eq!(*supervisor.launches.lock().unwrap(), 0);
}
