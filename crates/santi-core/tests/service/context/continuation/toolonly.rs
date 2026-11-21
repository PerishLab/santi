use super::*;

#[tokio::test]
async fn active() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let mut events = tools();
    events.retain(|event| !matches!(event, Event::Text(_)));
    let provider = Arc::new(Script::new(vec![
        events,
        finish("answer"),
        finish("resumed"),
    ]));
    let service = Service::open(crate::SCENARIO, config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let entries = store.entries(&strand.id).await.unwrap();
    let first = entries
        .iter()
        .find(|entry| entry.kind == strand::Target::Thinking)
        .unwrap()
        .target
        .clone();
    let last = entries
        .iter()
        .rfind(|entry| entry.kind == strand::Target::ToolResult)
        .unwrap()
        .target
        .clone();
    super::preview::opened(&store, &strand.id).await;
    let request = compact::Exec {
        first: Some(first.clone()),
        last: Some(last.clone()),
        from: None,
        to: None,
        summary: "the earlier tools completed".into(),
        absorb: Vec::new(),
        capsule: None,
        dry: true,
    };
    let preview = service.exec(&strand.id, request.clone()).await.unwrap();
    assert!(preview.dry && store.compacts(&strand.id).await.unwrap().is_empty());
    let report = service
        .exec(
            &strand.id,
            compact::Exec {
                dry: false,
                ..request
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (report.first.as_str(), report.last.as_str()),
        (first.as_str(), last.as_str())
    );
    assert!(report.after.is_some());
    let input = santi_core::provider_input(&store, &strand.id)
        .await
        .unwrap();
    assert!(!originals(&input).contains(&"original 工具🙂".into()));
    assert!(originals(&input).contains(&"active reasoning".into()));
    assert!(
        input
            .iter()
            .any(|item| matches!(item, Item::Call {call,..} if call == "call_active"))
    );
    let audit = service.audit(&strand.id).await.unwrap().unwrap();
    assert!(audit.request.estimate.total > 0);
    let slots = audit.slots.expect("entry-bound compacts occupy slots");
    assert_eq!(slots.held.len(), 1);
    assert_eq!(
        (slots.held[0].from, slots.held[0].to),
        (report.from, report.to)
    );
    let page = service
        .page(&report.compact, None, 0, 100)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.entries.first().unwrap().target, first);
    assert_eq!(page.entries.last().unwrap().target, last);
    assert!(
        page.entries
            .iter()
            .all(|entry| entry.kind != strand::Target::Message)
    );

    super::preview::closed(&store, &strand.id).await;
    send(&service, &strand.id).await;
    {
        let requests = provider.requests.lock().unwrap();
        let resumed = requests.last().unwrap();
        provider
            .validate(resumed)
            .expect("continuation still matches the live calls and results");
        assert!(originals(&resumed.input).contains(&"active reasoning".into()));
        for name in ["call_active", "call_second"] {
            assert!(
                resumed
                    .input
                    .iter()
                    .any(|item| matches!(item, Item::Output {call,..} if call == name))
            );
        }
    }
    let child = service.fork(&strand.id).await.unwrap().strand;
    let inherited = santi_core::provider_input(&store, &child.id).await.unwrap();
    assert!(!originals(&inherited).contains(&"original 工具🙂".into()));
    assert!(inherited.iter().any(|item| matches!(item, Item::Message {content,..} if content.contains("the earlier tools completed"))));
}
