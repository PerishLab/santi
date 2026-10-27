use super::*;

#[tokio::test]
async fn compact() {
    let fixture = Fixture::open().await;
    fixture.load(Scope::Strand, "load").await;
    let runtime = fixture.send("remember this").await;
    let first = runtime.messages.first().unwrap().message.id.clone();
    let last = runtime.messages.last().unwrap().message.id.clone();
    fixture
        .service
        .exec(
            &fixture.strand,
            santi_core::compact::Exec {
                first: Some(first),
                last: Some(last),
                from: None,
                to: None,
                summary: "one settled exchange".into(),
                absorb: vec![],
                capsule: None,
                dry: false,
            },
        )
        .await
        .unwrap();
    assert!(fixture.prompt().await.contains("first-marker"));
    let next = fixture.send("continue").await;
    assert_eq!(next.contexts.len(), 2);
    assert_eq!(
        next.contexts[0].materials[0].revision,
        next.contexts[1].materials[0].revision
    );
}

#[tokio::test]
async fn fork() {
    let fixture = Fixture::open().await;
    fixture.load(Scope::Strand, "load").await;
    let runtime = fixture.send("remember this").await;
    let child = fixture.service.fork(&fixture.strand).await.unwrap().strand;
    fixture
        .service
        .drop(Scope::Strand, &fixture.strand, command("drop_parent"))
        .await
        .unwrap();
    let material::View::Blob(blob) = fixture
        .service
        .browse(
            Scope::Strand,
            &child.id,
            material::Query {
                context: Some(runtime.contexts[0].id.clone()),
                reference: Some("perish:plumb".into()),
                blob: Some("SKILL.md".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(blob.text.contains("first-marker"));
    let material::View::State(state) = fixture
        .service
        .browse(Scope::Strand, &child.id, material::Query::default())
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(state.bindings.len(), 1);
    fixture
        .service
        .drop(Scope::Strand, &child.id, command("drop_child"))
        .await
        .unwrap();
    let store = santi_core::Store::open(config(&fixture.temp).database)
        .await
        .unwrap();
    store.end_strand(&child.id).await.unwrap();
    store.end_strand(&fixture.strand).await.unwrap();
}
