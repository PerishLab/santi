use super::*;

#[tokio::test]
async fn layers() {
    let fixture = Fixture::open().await;
    let first = package("first guide");
    let second = package("second guide");
    fixture.bind(Scope::Soul, "shared", &first).await.unwrap();
    fixture.bind(Scope::Strand, "local", &first).await.unwrap();
    let selection = fixture.selection("first").await;
    assert_eq!(selection.bindings.len(), 2);
    assert_eq!(selection.packages.len(), 1);
    assert_eq!(
        fixture.selection("second").await.packages,
        vec![first.clone()]
    );
    assert!(
        fixture
            .bind(Scope::Strand, "conflict", &second)
            .await
            .unwrap_err()
            .contains("conflicts")
    );
    assert!(
        fixture
            .bind(Scope::Soul, "conflict", &second)
            .await
            .unwrap_err()
            .contains("conflicts")
    );
    assert!(
        fixture
            .store
            .operation(Scope::Soul, "soul", "conflict")
            .await
            .unwrap()
            .is_none()
    );
    fixture.drop(Scope::Strand, "drop_local").await;
    assert_eq!(fixture.selection("first").await.bindings.len(), 1);
    fixture.bind(Scope::Soul, "refresh", &second).await.unwrap();
    assert_eq!(fixture.selection("first").await.packages, vec![second]);
    fixture.drop(Scope::Soul, "drop_shared").await;
    assert!(fixture.selection("first").await.packages.is_empty());
}

#[tokio::test]
async fn operations() {
    let fixture = Fixture::open().await;
    let first = package("first guide");
    let second = package("second guide");
    let original = fixture.bind(Scope::Strand, "old", &first).await.unwrap();
    fixture.bind(Scope::Strand, "new", &second).await.unwrap();
    let repeated = fixture.bind(Scope::Strand, "old", &first).await.unwrap();
    assert_eq!(repeated.id, original.id);
    assert_eq!(
        fixture.selection("first").await.packages,
        vec![second.clone()]
    );
    assert!(
        fixture
            .bind(Scope::Strand, "old", &second)
            .await
            .unwrap_err()
            .contains("conflicts")
    );
    fixture.drop(Scope::Strand, "drop").await;
    fixture.bind(Scope::Strand, "newest", &first).await.unwrap();
    fixture.drop(Scope::Strand, "drop").await;
    assert_eq!(fixture.selection("first").await.packages.len(), 1);
    let path = fixture.temp.path().join("estate.sqlite");
    let reopened = Store::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .bindings(Scope::Strand, "first")
            .await
            .unwrap()
            .packages,
        vec![first]
    );
    assert_eq!(
        reopened
            .operation(Scope::Strand, "first", "old")
            .await
            .unwrap()
            .unwrap()
            .id,
        original.id
    );
}

#[tokio::test]
async fn forks() {
    let fixture = Fixture::open().await;
    let first = package("first guide");
    fixture.bind(Scope::Strand, "local", &first).await.unwrap();
    let child = fixture
        .store
        .fork(santi_estate::ForkDraft {
            tag: "child",
            parent: "first",
            at: 0,
            created: NOW,
        })
        .await
        .unwrap();
    assert_eq!(
        fixture.selection(&child.id).await.packages,
        vec![first.clone()]
    );
    fixture.drop(Scope::Strand, "drop").await;
    assert_eq!(
        fixture.selection(&child.id).await.packages,
        vec![first.clone()]
    );
    fixture.store.end_strand(&child.id).await.unwrap();
    fixture.store.end_strand("first").await.unwrap();
    fixture
        .bind(Scope::Soul, "remaining", &first)
        .await
        .unwrap();
    assert_eq!(fixture.selection("second").await.packages, vec![first]);
}

#[tokio::test]
async fn concurrent() {
    let fixture = Fixture::open().await;
    let first = package("first guide");
    let second = package("second guide");
    let (shared, local) = tokio::join!(
        fixture.bind(Scope::Soul, "shared", &first),
        fixture.bind(Scope::Strand, "local", &second)
    );
    assert_ne!(shared.is_ok(), local.is_ok());
    let selected = fixture.selection("first").await;
    assert_eq!(selected.packages.len(), 1);
    assert_eq!(selected.bindings.len(), 1);
}
