use axum::body::to_bytes;
use axum::response::IntoResponse;
use sqlx::{Connection, SqliteConnection};

use super::*;

async fn answer(error: ApiError) -> (StatusCode, String) {
    let response = error.into_response();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 16).await.expect("body");
    (status, String::from_utf8(body.to_vec()).expect("utf8"))
}

async fn aside(database: &std::path::Path) -> SqliteConnection {
    SqliteConnection::connect(&format!("sqlite://{}", database.display()))
        .await
        .expect("second connection")
}

fn draft<'a>(tag: &'a str, created: &'a str) -> StrandDraft<'a> {
    StrandDraft {
        tag,
        soul: santi_core::GENESIS,
        label: None,
        parent: None,
        fork: None,
        created,
    }
}

async fn open(temp: &tempfile::TempDir) -> (std::path::PathBuf, santi_core::Store) {
    let database = temp.path().join("santi.sqlite");
    let store = support::bootstrap(&database).await;
    store
        .seed(santi_core::GENESIS, &santi_core::now())
        .await
        .expect("seed");
    (database, store)
}

fn shape(body: &str) -> (String, String, String) {
    let value: serde_json::Value = serde_json::from_str(body).expect("json body");
    let text = |key: &str| value[key].as_str().expect(key).to_string();
    (text("code"), text("message"), text("category"))
}

fn shaped(code: &str, message: &str, category: &str) -> (String, String, String) {
    (code.to_string(), message.to_string(), category.to_string())
}

#[tokio::test]
async fn taken() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (database, store) = open(&temp).await;
    let mut aside = aside(&database).await;
    sqlx::query("CREATE UNIQUE INDEX clash ON soul_strand (created)")
        .execute(&mut aside)
        .await
        .expect("unique index");
    let created = santi_core::now();
    store
        .create_strand(draft("ss_first", &created))
        .await
        .expect("first strand");
    let error = store
        .create_strand(draft("ss_second", &created))
        .await
        .expect_err("unique violation");
    let (status, body) = answer(ApiError::from_service(error)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        shape(&body),
        shaped("request.invalid_argument", "key taken", "invalid")
    );
    assert!(!body.contains("created"), "{body}");
    assert!(!body.contains("UNIQUE"), "{body}");
}

#[tokio::test]
async fn down() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (database, store) = open(&temp).await;
    let mut held = aside(&database).await;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut held)
        .await
        .expect("hold the write lock");
    let created = santi_core::now();
    let error = store
        .create_strand(draft("ss_down", &created))
        .await
        .expect_err("blocked write");
    let (status, body) = answer(ApiError::from_service(error)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        shape(&body),
        shaped("runtime.unavailable", "store unavailable", "unavailable")
    );
    assert!(!body.contains("locked"), "{body}");
    sqlx::query("ROLLBACK")
        .execute(&mut held)
        .await
        .expect("release");
}

async fn service(temp: &tempfile::TempDir, database: &std::path::Path) -> Service {
    Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
    )
    .await
    .expect("open service")
}

#[tokio::test]
async fn fault() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (database, store) = open(&temp).await;
    drop(store);
    let service = service(&temp, &database).await;
    let mut aside = aside(&database).await;
    let tables = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&mut aside)
    .await
    .expect("tables");
    for table in tables {
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE \"{table}\"")))
            .execute(&mut aside)
            .await
            .expect("drop");
    }
    let error = effect_status_handler(State(service), Path("effect_fault".to_string()))
        .await
        .expect_err("broken store");
    let (status, body) = answer(error).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        shape(&body),
        shaped("runtime.internal", "store fault", "internal")
    );
    assert!(!body.contains("no such table"), "{body}");
}

#[tokio::test]
async fn other() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (database, store) = open(&temp).await;
    drop(store);
    let service = service(&temp, &database).await;
    let error = effect_status_handler(State(service), Path("effect_none".to_string()))
        .await
        .expect_err("missing effect");
    let (status, body) = answer(error).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        shape(&body),
        shaped("resource.not_found", "effect not found", "missing")
    );
}
