use santi_estate::Store;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{AssertSqlSafe, Connection, Executor};
use std::path::Path;

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SOUL: &str = "soul_hot";
const FIRST: &str = "2026-07-28T00:00:00.000Z";
const KEY: &str = "SANTI_429_HOT_DB";
const HELPER: &str = "transition::hot::helper";
const PAGES: usize = 192;
const PAGE: usize = 4096;

#[tokio::test]
async fn journal() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("bootstrap");
    let soul = store.seed(SOUL, FIRST).await.expect("seed");
    drop(store);

    let status = tokio::process::Command::new(std::env::current_exe().expect("current exe"))
        .arg("--exact")
        .arg(HELPER)
        .env(KEY, &path)
        .status()
        .await
        .expect("helper");
    assert!(status.success());

    let journal = path.with_file_name("estate.sqlite-journal");
    assert!(std::fs::metadata(&journal).expect("hot journal").len() >= 512);

    let store = Store::open(&path)
        .await
        .expect("open estate with hot journal");
    let retained = store
        .soul(SOUL)
        .await
        .expect("souls")
        .expect("committed soul");
    assert_eq!(retained.id, soul.id);
    assert_eq!(store.souls().await.expect("souls").len(), 1);
    drop(store);
    assert_eq!(scratch(&path).await, 0);
}

#[tokio::test]
async fn helper() {
    let Ok(path) = std::env::var(KEY) else {
        return;
    };
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(false)
        .pragma("cache_size", "-16")
        .pragma("journal_mode", "delete");
    let mut conn = SqliteConnection::connect_with(&options)
        .await
        .expect("helper connection");
    conn.execute(sqlx::raw_sql(AssertSqlSafe("BEGIN IMMEDIATE".to_string())))
        .await
        .expect("begin");
    conn.execute(sqlx::raw_sql(AssertSqlSafe(
        "CREATE TABLE scratch (body BLOB NOT NULL)".to_string(),
    )))
    .await
    .expect("scratch");
    let body = vec![0x5au8; PAGE];
    for _ in 0..PAGES {
        sqlx::query("INSERT INTO scratch (body) VALUES (?)")
            .bind(body.as_slice())
            .execute(&mut conn)
            .await
            .expect("spill");
    }
    std::process::exit(0);
}

async fn scratch(path: &Path) -> i64 {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let mut conn = SqliteConnection::connect_with(&options)
        .await
        .expect("verify connection");
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'scratch'",
    )
    .fetch_one(&mut conn)
    .await
    .expect("scratch lookup")
}
