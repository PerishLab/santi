use super::*;

#[tokio::test]
async fn pinned() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let resolver = Arc::new(Switch {
        current: Mutex::new(effective("a", "a-model", "low")),
        a: Arc::new(Held {
            provider: "a",
            model: "a-model",
            bytes: 20_000,
            maintenance: 30_000,
        }),
        b: Arc::new(Held {
            provider: "b",
            model: "b-model",
            bytes: 40_000,
            maintenance: 60_000,
        }),
    });
    let service = Service::provided(
        config.clone(),
        resolver.clone(),
        Declaration::default(),
        Arc::new(Jobs),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    let content = message::Content::text("hold this turn open");
    store
        .accept_inbox(
            santi_core::InboxDraft {
                tag: &santi_core::tag("inbox"),
                strand: &strand.id,
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .unwrap();
    service.drive(&strand.id).await.unwrap();

    let mut running = false;
    for _ in 0..100 {
        if let Some(turn) = store.latest(&strand.id).await.unwrap()
            && turn.status == santi_core::turn::Status::Running
        {
            running = true;
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert!(running, "the blocking provider did not enter Running");

    *resolver.current.lock().unwrap() = effective("b", "b-model", "high");
    let patch = Patch {
        provider: Some(Some("b".into())),
        model: Some(Some("b-model".into())),
        reasoning_effort: Some(Some("high".into())),
    };
    store
        .declare(Target::Strand, &strand.id, &patch, &santi_core::now())
        .await
        .unwrap();

    let mut budget = None;
    for _ in 0..100 {
        let audit = service.audit(&strand.id).await.unwrap().unwrap();
        if audit.budget.as_ref().map(|budget| budget.bytes) == Some(20_000) {
            budget = audit.budget;
            assert_eq!(budget.as_ref().unwrap().maintenance, Some(30_000));
            assert_eq!(audit.request.ceiling, Some(30_000));
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(budget.expect("pinned budget").bytes, 20_000);

    let active = service
        .running(&strand.soul, None)
        .await
        .unwrap()
        .expect("active projection");
    assert_eq!(active.active.len(), 1);
    let budget = active.active[0].budget.clone().expect("pinned budget");
    assert_eq!(budget.bytes, 20_000);
    assert_eq!(budget.maintenance, Some(30_000));
}
