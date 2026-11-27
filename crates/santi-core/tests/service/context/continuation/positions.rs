use super::super::*;
use santi_estate::Compact;

#[tokio::test]
async fn projection() {
    let temp = tempfile::tempdir().expect("temp");
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![
        tools(),
        finish("first"),
        finish("second"),
        finish("third"),
        finish("child"),
    ]));
    let service = Service::open(crate::SCENARIO, config(&temp), provider.clone())
        .await
        .expect("service");
    let parent = service.weave().await.expect("parent").strand;
    send(&service, &parent.id).await;
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .expect("store");
    let messages = store.messages(&parent.id).await.expect("messages");
    let offset = messages.len();
    let inside = store
        .condense(Compact {
            tag: "compact_inside",
            strand: &parent.id,
            first: &messages.first().expect("first").message.id,
            last: &messages.last().expect("last").message.id,
            summary: "inside",
            metadata: None,
            expected: None,
            created: &santi_core::now(),
        })
        .await
        .expect("inside compact");
    send(&service, &parent.id).await;
    let at = store
        .strand(&parent.id)
        .await
        .expect("parent")
        .expect("held")
        .next
        - 1;
    let expected = santi_core::assemble(&store, &parent.id)
        .await
        .expect("prefix");
    send(&service, &parent.id).await;
    let messages = store.messages(&parent.id).await.expect("messages");
    store
        .condense(Compact {
            tag: "compact_crossing",
            strand: &parent.id,
            first: &messages[offset].message.id,
            last: &messages.last().expect("last").message.id,
            summary: "crossing",
            metadata: None,
            expected: None,
            created: &santi_core::now(),
        })
        .await
        .expect("crossing compact");
    let home = std::path::PathBuf::from(config(&temp).runtime).join("strands");
    let current = home.join(&parent.id).join("memory");
    std::fs::create_dir_all(&current).expect("workspace");
    std::fs::write(current.join("marker"), "current workspace").expect("latest file");
    let child = service
        .branching(&parent.id, Some(at), None)
        .await
        .expect("historical fork");
    assert_eq!(child.workspace, strand::Workspace::Current);
    assert_eq!(child.strand.fork, Some(at));
    assert_eq!(child.strand.next, at + 1);
    assert_eq!(child.strand.inference, parent.inference);
    let actual = santi_core::assemble(&store, &child.strand.id)
        .await
        .expect("child projection");
    let inherited = store.compacts(&child.strand.id).await.expect("compacts");
    assert_eq!(inherited.len(), 1);
    assert_eq!(inherited[0].summary, "inside");
    assert_ne!(inherited[0].id, inside.compact);
    assert_eq!(
        format!("{actual:?}"),
        format!("{expected:?}")
            .replace(&inside.compact, &inherited[0].id)
            .replace(&parent.id, &child.strand.id)
    );
    assert_eq!(
        std::fs::read_to_string(home.join(&child.strand.id).join("memory/marker"))
            .expect("copied file"),
        "current workspace"
    );
    let latest = store
        .strand(&parent.id)
        .await
        .expect("parent")
        .expect("held")
        .next
        - 1;
    let head = service.fork(&parent.id).await.expect("default fork");
    assert_eq!(head.strand.fork, Some(latest));
    assert_eq!(
        store
            .compacts(&head.strand.id)
            .await
            .expect("head compacts")
            .len(),
        2
    );
    send(&service, &child.strand.id).await;
    assert_eq!(
        originals(
            &provider
                .requests
                .lock()
                .expect("requests")
                .last()
                .expect("child request")
                .input
        ),
        ["original second"]
    );
}

#[tokio::test]
async fn refusal() {
    let temp = tempfile::tempdir().expect("temp");
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![tools(), finish("settled")]));
    let service = Service::open(crate::SCENARIO, config(&temp), provider.clone())
        .await
        .expect("service");
    let parent = service.weave().await.expect("parent").strand;
    send(&service, &parent.id).await;
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .expect("store");
    let head = store
        .strand(&parent.id)
        .await
        .expect("parent")
        .expect("held")
        .next
        - 1;
    super::opened(&store, &parent.id, head).await;
    let partial = store
        .strand(&parent.id)
        .await
        .expect("parent")
        .expect("held")
        .next
        - 1;
    let before = store.strands().await.expect("strands").len();
    let error = service
        .branching(&parent.id, Some(partial), None)
        .await
        .expect_err("unfinished exchange");
    assert!(
        error.contains(&format!("earlier valid position is {head}")),
        "{error}"
    );
    assert!(error.contains("later valid position is none"), "{error}");
    assert_eq!(store.strands().await.expect("strands").len(), before);
    assert_eq!(provider.requests.lock().expect("requests").len(), 2);
    assert_eq!(
        service
            .fork(&parent.id)
            .await
            .expect("default checkpoint")
            .strand
            .fork,
        Some(head)
    );
}
