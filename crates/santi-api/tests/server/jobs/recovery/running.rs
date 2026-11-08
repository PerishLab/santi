use super::super::*;
use super::support::{Gated, Terminal, boot, origin, terminal};
use santi_core::{message, strand};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn running() {
    let temp = tempfile::tempdir().expect("temp dir");
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let provider = Arc::new(Gated::new());
    let (service, store, strand) = boot(&temp, provider.clone(), supervisor.clone()).await;
    origin(&store, &strand, "turn_origin").await;

    let sent = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "do the work".to_string(),
                }],
            },
        )
        .await
        .expect("send");
    tokio::time::timeout(Duration::from_secs(10), provider.entered.notified())
        .await
        .expect("first provider round entered");
    let running = store
        .latest(&strand.id)
        .await
        .expect("latest")
        .expect("running turn");
    assert_eq!(running.status, santi_core::turn::Status::Running);
    assert_ne!(running.id, "turn_origin");
    assert_eq!(store.running().await.expect("running count"), 1);

    let job = terminal(Terminal {
        store: &store,
        strand: &strand,
        job: "job_live",
        notice: "inbox_live",
        text: "terminal fact live",
    })
    .await;
    assert_eq!(store.inboxes(&strand.id).await.expect("pending").len(), 1);

    service.resume().await.expect("resume while running");
    let after = store
        .latest(&strand.id)
        .await
        .expect("latest")
        .expect("turn");
    assert_eq!(after.id, running.id);
    assert_eq!(after.status, santi_core::turn::Status::Running);
    assert_eq!(store.turns(&strand.id).await.expect("turns").len(), 2);
    assert_eq!(store.inboxes(&strand.id).await.expect("pending").len(), 1);

    provider.release.notify_one();

    let mut recovered = false;
    for _ in 0..500 {
        if provider.requests.lock().unwrap().len() >= 2 {
            recovered = true;
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(
        recovered,
        "turn completion did not consume the terminal notice"
    );

    let mut finished = None;
    for _ in 0..500 {
        let turns = store.turns(&strand.id).await.expect("turns");
        if turns.len() == 3
            && turns
                .iter()
                .all(|turn| turn.status == santi_core::turn::Status::Completed)
        {
            finished = turns
                .iter()
                .find(|turn| turn.id != "turn_origin" && turn.id != running.id)
                .map(|turn| turn.id.clone());
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let finished = finished.expect("completion poke did not finish the second turn");

    {
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2, "expected exactly two provider rounds");
        let messages: Vec<&str> = requests[1]
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
                .any(|content| content.contains("terminal fact live")),
            "second provider request is missing the terminal notice: {messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|content| content.contains("causes: [\"terminal\"]")),
            "second provider request is missing the terminal cause: {messages:?}"
        );
    }

    assert!(
        store.inboxes(&strand.id).await.expect("drained").is_empty(),
        "terminal notice remained pending after turn completion"
    );
    let record = store.job_record(&job).await.expect("record").expect("job");
    assert_eq!(record.job.state, job::State::Succeeded);
    assert!(record.terminal_notified.is_some());
    assert_eq!(*supervisor.launches.lock().unwrap(), 0);
    assert_ne!(finished, running.id);
    let _ = sent;
}
