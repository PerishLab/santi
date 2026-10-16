use super::*;
use santi_core::Store;
use std::fs;

async fn open(temp: &tempfile::TempDir, provider: Arc<dyn Provider>) -> Service {
    bootstrap(temp).await;
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
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

fn memory(temp: &tempfile::TempDir, text: &str) -> std::path::PathBuf {
    let path = temp
        .path()
        .join("runtime/souls/soul_default/memory/MEMORY.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn post() -> strand::Post {
    strand::Post {
        content: vec![message::Part::Text {
            text: "continue work".into(),
        }],
    }
}

fn requests(provider: &BudgetedProvider, strand: &str) -> usize {
    provider
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| {
            request
                .instructions
                .as_deref()
                .unwrap()
                .contains(&format!("strand: {strand}\n"))
        })
        .count()
}

#[tokio::test]
async fn bytes() {
    let temp = tempfile::tempdir().unwrap();
    let provider = Arc::new(BudgetedProvider::default());
    let service = open(&temp, provider.clone()).await;
    let source = memory(&temp, &"x".repeat(service::MEMORY + 1));
    let strand = service.weave().await.unwrap().strand;
    let posted = service.send(&strand.id, post()).await.unwrap();
    assert!(posted.turn.is_none());
    assert_eq!(requests(&provider, &strand.id), 0);
    assert_eq!(fs::metadata(source).unwrap().len(), 32_769);
    assert_eq!(
        service
            .receipt(&posted.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
        santi_core::receipt::State::Accepted
    );
    for _ in 0..200 {
        if !provider.requests.lock().unwrap().is_empty() {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    let held = provider.requests.lock().unwrap().clone();
    assert_eq!(
        held.len(),
        1,
        "only the memory maintenance brain may request"
    );
    let tools = serde_json::to_string(held[0].tools.as_ref().unwrap()).unwrap();
    assert!(
        tools.contains("shell"),
        "maintenance can organize the source through its ordinary shell"
    );
    assert!(
        held[0]
            .instructions
            .as_deref()
            .unwrap()
            .contains("soul_memory_projection")
    );
    service.close();
    tokio::time::timeout(Duration::from_secs(5), service.drain())
        .await
        .unwrap();

    let temp = tempfile::tempdir().unwrap();
    let provider = Arc::new(BudgetedProvider::default());
    let service = open(&temp, provider.clone()).await.memory(6).unwrap();
    memory(&temp, "界界");
    let strand = service.weave().await.unwrap().strand;
    let posted = service.send(&strand.id, post()).await.unwrap();
    Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&service, &posted).await.id)
        .await;
    assert_eq!(requests(&provider, &strand.id), 1);
    let source = memory(&temp, "界界a");
    let posted = service.send(&strand.id, post()).await.unwrap();
    assert!(posted.turn.is_none());
    assert_eq!(requests(&provider, &strand.id), 1);
    assert_eq!(fs::read_to_string(source).unwrap(), "界界a");
    service.close();
    tokio::time::timeout(Duration::from_secs(5), service.drain())
        .await
        .unwrap();
}

#[tokio::test]
async fn growth() {
    let temp = tempfile::tempdir().unwrap();
    let source = memory(&temp, "界界");
    let provider = Arc::new(BudgetedProvider {
        rounds: 1,
        command: Some(format!("printf '%s' '界界a' > '{}'", source.display())),
        ..Default::default()
    });
    let service = open(&temp, provider.clone()).await.memory(6).unwrap();
    let strand = service.weave().await.unwrap().strand;
    let posted = service.send(&strand.id, post()).await.unwrap();
    let turn = accepted_turn(&service, &posted).await.id;
    let runtime = Probe::new(&service).completed_turn(&strand.id, &turn).await;
    assert_eq!(
        requests(&provider, &strand.id),
        1,
        "growth must pause before a second ordinary request"
    );
    assert_eq!(
        runtime.results.len(),
        1,
        "completed shell evidence survives pause"
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), "界界a");
    let store = Store::open(temp.path().join("santi.sqlite")).await.unwrap();
    let inbox = store.inboxes(&strand.id).await.unwrap();
    assert!(
        inbox
            .iter()
            .any(|item| item.content.rendered().contains("kind: soul_memory_pause"))
    );
    assert_eq!(
        service
            .receipt(&posted.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
        santi_core::receipt::State::Completed
    );
    fs::write(&source, "界界").unwrap();
    service.resume().await.unwrap();
    for _ in 0..200 {
        if requests(&provider, &strand.id) == 2 {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        requests(&provider, &strand.id),
        2,
        "ordinary brain continues through its queued handoff"
    );
    service.close();
    tokio::time::timeout(Duration::from_secs(5), service.drain())
        .await
        .unwrap();
}
