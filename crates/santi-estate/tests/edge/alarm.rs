use santi_estate::{Store, StrandDraft};
use santi_model::alarm::{Draft, Occurrence};

const NOW: &str = "2026-10-19T06:00:00Z";

async fn estate(temp: &tempfile::TempDir) -> Store {
    let path = temp.path().join("estate");
    Store::bootstrap(
        &path,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let store = Store::open(&path).await.unwrap();
    store.seed("adam", NOW).await.unwrap();
    store
        .create_strand(StrandDraft {
            tag: "brain",
            soul: "adam",
            label: None,
            parent: None,
            fork: None,
            created: NOW,
        })
        .await
        .unwrap();
    store
}

fn draft() -> Draft {
    Draft {
        soul: "adam".into(),
        strand: "brain".into(),
        name: "follow".into(),
        text: "查看进展\n$() % '保留原文'".into(),
        cron: "*/15 * * * *".into(),
        generation: None,
    }
}

#[tokio::test]
async fn declaration() {
    let temp = tempfile::tempdir().unwrap();
    let store = estate(&temp).await;
    let mut input = draft();
    let first = store.set_alarm(&input).await.unwrap();
    let same = store.set_alarm(&input).await.unwrap();
    assert_eq!(
        (first.id.clone(), first.generation),
        (same.id, same.generation)
    );
    input.text = "new text".into();
    assert!(store.set_alarm(&input).await.is_err());
    input.generation = Some(1);
    let changed = store.set_alarm(&input).await.unwrap();
    assert_eq!(changed.generation, 2);
    input.generation = Some(1);
    input.text = "stale text".into();
    assert!(store.set_alarm(&input).await.is_err());
    input.soul = "foreign".into();
    assert!(store.set_alarm(&input).await.is_err());
    assert!(store.end_alarm(&first.id, "foreign").await.is_err());
    store.end_alarm(&first.id, "adam").await.unwrap();
    assert!(store.alarms().await.unwrap().is_empty());
}

#[tokio::test]
async fn receipts() {
    let temp = tempfile::tempdir().unwrap();
    let store = estate(&temp).await;
    let input = draft();
    let alarm = store.set_alarm(&input).await.unwrap();
    let occurrence = Occurrence {
        generation: 1,
        minute: 29_000_000,
    };
    let first = store.accept_alarm(&alarm.id, &occurrence).await.unwrap();
    let replay = store.accept_alarm(&alarm.id, &occurrence).await.unwrap();
    assert!(first.inserted);
    assert!(!replay.inserted);
    assert_eq!(first.receipt.inbox, replay.receipt.inbox);
    let pending = store.inboxes("brain").await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].content.rendered(), input.text);
    assert_eq!(pending[0].source.as_ref().unwrap().kind, "alarm");
    let mut next = occurrence.clone();
    next.minute += 1;
    let second = store.accept_alarm(&alarm.id, &next).await.unwrap();
    assert_ne!(first.receipt.inbox, second.receipt.inbox);
    assert_eq!(store.inboxes("brain").await.unwrap().len(), 2);
    store.end_alarm(&alarm.id, "adam").await.unwrap();
    assert!(store.receipt(&first.receipt.inbox).await.unwrap().is_some());
    drop(store);
    let store = Store::open(temp.path().join("estate")).await.unwrap();
    assert_eq!(
        store
            .accept_alarm(&alarm.id, &occurrence)
            .await
            .unwrap()
            .receipt
            .inbox,
        first.receipt.inbox
    );
    next.minute += 1;
    assert!(store.accept_alarm(&alarm.id, &next).await.is_err());
    assert_eq!(store.inboxes("brain").await.unwrap().len(), 2);
}

#[tokio::test]
async fn generation() {
    let temp = tempfile::tempdir().unwrap();
    let store = estate(&temp).await;
    let mut input = draft();
    let alarm = store.set_alarm(&input).await.unwrap();
    input.text = "changed".into();
    input.generation = Some(1);
    store.set_alarm(&input).await.unwrap();
    let mut occurrence = Occurrence {
        generation: 1,
        minute: 29_000_000,
    };
    assert!(store.accept_alarm(&alarm.id, &occurrence).await.is_err());
    assert!(store.inboxes("brain").await.unwrap().is_empty());
    occurrence.generation = 2;
    store.accept_alarm(&alarm.id, &occurrence).await.unwrap();
    assert_eq!(
        store.inboxes("brain").await.unwrap()[0].content.rendered(),
        "changed"
    );
}

#[tokio::test]
async fn concurrent() {
    let temp = tempfile::tempdir().unwrap();
    let store = estate(&temp).await;
    let alarm = store.set_alarm(&draft()).await.unwrap();
    let occurrence = Occurrence {
        generation: 1,
        minute: 29_000_000,
    };
    let (first, second) = tokio::join!(
        store.accept_alarm(&alarm.id, &occurrence),
        store.accept_alarm(&alarm.id, &occurrence)
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_ne!(first.inserted, second.inserted);
    assert_eq!(first.receipt.inbox, second.receipt.inbox);
    assert_eq!(store.inboxes("brain").await.unwrap().len(), 1);
}
