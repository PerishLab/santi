use keel::adapt::db::Sqlite;
use keel::atom::string;
use santi_estate::{Bootstrap, Status, Store};

#[keel::resource]
struct Environ {
    #[field(string)]
    scope: string,
}

#[tokio::test]
async fn explicit() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");

    let error = match Store::open(&path).await {
        Ok(_) => panic!("vacant estate must refuse ordinary bind"),
        Err(error) => error,
    };
    assert!(error.contains("vacant estate needs bootstrap"));

    let mut estate = Bootstrap::open(&path).await.expect("bootstrap");
    assert_eq!(estate.status().await.expect("status"), Status::Vacant);
    let sudo = estate.mint().await.expect("mint");
    let store = estate.seal(&sudo).await.expect("seal");
    store
        .seed("soul_test", "2026-07-31T00:00:00Z")
        .await
        .expect("seed");
    drop(store);

    Store::open(&path).await.expect("ordinary bind");
    Store::bootstrap(&path, &sudo)
        .await
        .expect("exact bootstrap replay");
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(Store::bootstrap(&path, wrong).await.is_err());
}

#[tokio::test]
async fn earlier() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let mut graph = santi_estate::graph();
    graph.plug::<Environ>();
    keel::bootstrap(graph, Sqlite::file(&path).await.expect("wire"))
        .expect("plan")
        .seal("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .await
        .expect("bootstrap");
    let refused = Store::open(&path).await.err().expect("refused");
    assert!(refused.contains("earlier santi model"), "{refused}");
}
