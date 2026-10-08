use super::*;

#[tokio::test]
async fn waits() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = super::super::support::bootstrap(&path).await;
    store
        .seed("retained", "2026-10-08T00:00:00.000Z")
        .await
        .expect("seed");
    let mut writer = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .expect("writer");
    writer
        .execute("BEGIN EXCLUSIVE")
        .await
        .expect("hold writer");

    let opening = Store::open(&path);
    tokio::pin!(opening);
    match tokio::time::timeout(std::time::Duration::from_millis(100), &mut opening).await {
        Err(_) => {}
        Ok(Err(error)) => panic!("current estate refused transient writer: {error}"),
        Ok(Ok(_)) => panic!("current estate opened through an exclusive writer"),
    }
    writer.execute("ROLLBACK").await.expect("release writer");
    let reopened = tokio::time::timeout(std::time::Duration::from_secs(5), opening)
        .await
        .expect("bounded open")
        .expect("reopen current estate");
    assert!(
        reopened
            .soul("retained")
            .await
            .expect("retained soul")
            .is_some()
    );
    assert!(!Estate(&path).root().exists());
    writer.close().await.expect("close writer");
}

#[tokio::test]
async fn refuses() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    fixture(&path, VERSION).await;
    let mut writer = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .expect("writer");
    writer
        .execute("BEGIN IMMEDIATE")
        .await
        .expect("hold legacy writer");
    let before = std::fs::read(&path).expect("original legacy database");
    let error = match Store::open(&path).await {
        Ok(_) => panic!("busy legacy estate must refuse transition"),
        Err(error) => error,
    };
    assert!(error.contains("legacy database must be closed before transition"));
    assert_eq!(std::fs::read(&path).expect("preserved database"), before);
    assert!(!Estate(&path).root().exists());
    writer.execute("ROLLBACK").await.expect("release writer");
    writer.close().await.expect("close writer");
}
