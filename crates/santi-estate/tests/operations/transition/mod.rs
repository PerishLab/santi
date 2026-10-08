use santi_estate::Store;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{AssertSqlSafe, Connection, Executor};
use std::path::{Path, PathBuf};

const VERSION: i64 = 39;
const LEGACY: &str = r#"
CREATE TABLE compacts (id INTEGER);
CREATE TABLE downstream_ingest (id INTEGER);
CREATE TABLE downstreams (id INTEGER);
CREATE TABLE error_incidents (id INTEGER);
CREATE TABLE error_transitions (id INTEGER);
CREATE TABLE inbox_receipts (id INTEGER);
CREATE TABLE inbox_slots (id INTEGER);
CREATE TABLE job_capabilities (id INTEGER);
CREATE TABLE jobs (id INTEGER);
CREATE TABLE message_events (id INTEGER);
CREATE TABLE messages (id INTEGER);
CREATE TABLE provider_replay_material (id INTEGER);
CREATE TABLE r_strand_entries (id INTEGER);
CREATE TABLE receipt_transitions (id INTEGER);
CREATE TABLE souls (id INTEGER);
CREATE TABLE strand_effects (id INTEGER);
CREATE TABLE strand_inbox (id INTEGER);
CREATE TABLE strands (id INTEGER);
CREATE TABLE thinking_spans (id INTEGER);
CREATE TABLE tool_calls (id INTEGER);
CREATE TABLE tool_results (id INTEGER);
CREATE TABLE trace_records (id INTEGER);
CREATE TABLE turn_outbox (id INTEGER);
CREATE TABLE turn_stops (id INTEGER);
CREATE TABLE turns (id INTEGER);
CREATE TABLE webhook_deliveries (id INTEGER);
CREATE TABLE webhooks (id INTEGER);

CREATE INDEX idx_compacts_strand ON compacts(id);
CREATE INDEX idx_downstream_ingest_receipt ON downstream_ingest(id);
CREATE INDEX idx_error_incidents_active_key ON error_incidents(id);
CREATE INDEX idx_error_incidents_scope_time ON error_incidents(id);
CREATE INDEX idx_error_transitions_pending ON error_transitions(id);
CREATE INDEX idx_inbox_receipts_strand_state ON inbox_receipts(id);
CREATE INDEX idx_job_capabilities_expiry ON job_capabilities(id);
CREATE INDEX idx_jobs_soul_time ON jobs(id);
CREATE INDEX idx_jobs_state_time ON jobs(id);
CREATE INDEX idx_message_events_message_id_created_at ON message_events(id);
CREATE INDEX idx_messages_actor_created_at ON messages(id);
CREATE INDEX idx_messages_state_created_at ON messages(id);
CREATE INDEX idx_r_strand_entries_seq ON r_strand_entries(id);
CREATE INDEX idx_r_strand_entries_target_lookup ON r_strand_entries(id);
CREATE INDEX idx_receipt_transitions_receipt_time ON receipt_transitions(id);
CREATE INDEX idx_strand_effects_state_updated_at ON strand_effects(id);
CREATE INDEX idx_strand_effects_strand_created_at ON strand_effects(id);
CREATE INDEX idx_strand_effects_turn_created_at ON strand_effects(id);
CREATE INDEX idx_strand_inbox_coalesce ON strand_inbox(id);
CREATE INDEX idx_strand_inbox_strand_created_at ON strand_inbox(id);
CREATE INDEX idx_strands_external_label ON strands(id);
CREATE INDEX idx_strands_lineage ON strands(id);
CREATE INDEX idx_strands_soul_id ON strands(id);
CREATE INDEX idx_thinking_spans_turn_id_created_at ON thinking_spans(id);
CREATE INDEX idx_tool_calls_turn_id_created_at ON tool_calls(id);
CREATE INDEX idx_tool_results_tool_call_id ON tool_results(id);
CREATE INDEX idx_trace_records_name_opened_at ON trace_records(id);
CREATE INDEX idx_turn_outbox_label_seq ON turn_outbox(id);
CREATE INDEX idx_turn_outbox_seq ON turn_outbox(id);
CREATE INDEX idx_turns_strand_created_at ON turns(id);
CREATE INDEX idx_turns_strand_status_created_at ON turns(id);
CREATE INDEX idx_webhook_deliveries_receipt ON webhook_deliveries(id);
"#;
const RETIRED: &str = r#"
CREATE TABLE im_inbox (id INTEGER);
CREATE TABLE im_participants (id INTEGER);
CREATE TABLE r_soul_session_messages (id INTEGER);
CREATE INDEX idx_im_inbox_participant_seq ON im_inbox(id);
CREATE INDEX idx_im_inbox_turn ON im_inbox(id);
CREATE INDEX idx_r_soul_session_messages_seq ON r_soul_session_messages(id);
CREATE INDEX idx_r_soul_session_messages_target_lookup ON r_soul_session_messages(id);
"#;

struct Estate<'a>(&'a Path);

mod locks;

#[tokio::test]
async fn legacy() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    fixture(&path, VERSION).await;

    let store = super::support::bootstrap(&path).await;
    assert!(store.souls().await.expect("souls").is_empty());
    drop(store);

    let dirs = quarantines(&path);
    assert_eq!(dirs.len(), 1);
    let manifest = manifest(&dirs[0]);
    assert_eq!(manifest["state"], "ready");
    assert_eq!(manifest["legacy_version"], VERSION);
    assert!(dirs[0].join("estate.sqlite").exists());
    Store::open(&path).await.expect("reopen estate");
}

#[tokio::test]
async fn retired() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    fixture(&path, VERSION).await;
    execute(&path, RETIRED).await;

    let store = super::support::bootstrap(&path).await;
    assert!(store.souls().await.expect("souls").is_empty());
    drop(store);

    let dirs = quarantines(&path);
    assert_eq!(dirs.len(), 1);
    assert_eq!(manifest(&dirs[0])["state"], "ready");
    assert!(dirs[0].join("estate.sqlite").exists());
}

#[tokio::test]
async fn unknown() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("drift.sqlite");
    fixture(&path, VERSION).await;
    execute(&path, "CREATE TABLE unknown_resource (id INTEGER)").await;
    let error = match Store::open(&path).await {
        Ok(_) => panic!("drift must refuse"),
        Err(error) => error,
    };
    assert!(error.contains("shape is not exact"));
    assert!(path.exists());

    let path = temp.path().join("old.sqlite");
    fixture(&path, VERSION - 1).await;
    let error = match Store::open(&path).await {
        Ok(_) => panic!("old version must refuse"),
        Err(error) => error,
    };
    assert!(error.contains("unsupported legacy database version 38"));
    assert!(path.exists());
}

#[tokio::test]
async fn resumes() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    fixture(&path, VERSION).await;
    let root = Estate(&path).root();
    let moving = root.join(".moving-legacy-v39-test");
    std::fs::create_dir_all(&moving).expect("moving");
    let source = std::fs::canonicalize(temp.path())
        .expect("parent")
        .join("estate.sqlite");
    let held = serde_json::json!({
        "schema": "santi.legacy-quarantine.v1",
        "state": "moving",
        "legacy_version": VERSION,
        "source": source.display().to_string(),
        "created": "2026-07-28T00:00:00.000Z",
        "files": ["estate.sqlite"],
    });
    std::fs::write(
        moving.join("transition.json"),
        serde_json::to_vec_pretty(&held).expect("json"),
    )
    .expect("manifest");
    std::fs::rename(&path, moving.join("estate.sqlite")).expect("partial move");

    super::support::bootstrap(&path).await;
    let ready = root.join("legacy-v39-test");
    assert_eq!(manifest(&ready)["state"], "ready");
    assert!(ready.join("estate.sqlite").exists());
    assert!(path.exists());
}

#[tokio::test]
async fn orphans() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    std::fs::write(path.with_file_name("estate.sqlite-wal"), b"orphan").expect("sidecar");
    let error = match Store::open(&path).await {
        Ok(_) => panic!("orphan must refuse"),
        Err(error) => error,
    };
    assert!(error.contains("orphan SQLite sidecar"));
}

async fn fixture(path: &Path, version: i64) {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options)
        .await
        .expect("fixture db");
    conn.execute(sqlx::raw_sql(AssertSqlSafe(LEGACY.to_string())))
        .await
        .expect("legacy schema");
    conn.execute(sqlx::raw_sql(AssertSqlSafe(format!(
        "PRAGMA user_version = {version}"
    ))))
    .await
    .expect("version");
    conn.close().await.expect("close");
}

async fn execute(path: &Path, sql: &str) {
    let options = SqliteConnectOptions::new().filename(path);
    let mut conn = SqliteConnection::connect_with(&options)
        .await
        .expect("open fixture");
    conn.execute(sqlx::raw_sql(AssertSqlSafe(sql.to_string())))
        .await
        .expect("execute");
    conn.close().await.expect("close");
}

fn quarantines(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(Estate(path).root())
        .expect("quarantine")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("legacy-v39-"))
        })
        .collect()
}

fn manifest(dir: &Path) -> serde_json::Value {
    serde_json::from_slice(
        &std::fs::read(dir.join("transition.json")).expect("read transition manifest"),
    )
    .expect("transition manifest")
}

impl Estate<'_> {
    fn root(&self) -> PathBuf {
        self.0.with_file_name(format!(
            "{}.quarantine",
            self.0.file_name().expect("filename").to_string_lossy()
        ))
    }
}
