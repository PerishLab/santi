#[path = "jobs/server.rs"]
mod server;

use std::collections::HashMap;
use std::io::Cursor;

use santi::cli::ClientDefaults;
use santi::client::tui::{Request, session};
use serde_json::{Value, json};

fn job(id: &str, strand: &str) -> Value {
    json!({
        "id": id, "description": "detached\u{1b} work", "state": "succeeded", "exit_code": 0,
        "origin": {"soul": "soul_one", "strand": strand, "turn": "turn_one", "call": "call_one", "effect": "effect_one"}
    })
}

fn routes() -> HashMap<String, Value> {
    HashMap::from([
        (
            "/api/v1/strands/ss_one".into(),
            json!({"strand": {"id":"ss_one", "soul":"soul_one"}, "messages":[]}),
        ),
        ("/api/v1/souls/soul_one".into(), json!({"id":"soul_one"})),
        (
            "/api/v1/strands/ss_one/budget".into(),
            json!({"estimate":{"total":12},"budget":{"bytes":8000}}),
        ),
        (
            "/api/v1/jobs".into(),
            json!([job("job_other", "ss_other"), job("job_one", "ss_one")]),
        ),
        ("/api/v1/jobs/job_one".into(), job("job_one", "ss_one")),
        (
            "/api/v1/jobs/job_one/logs?stream=stdout&cursor=0&limit=4096".into(),
            json!({"job":"job_one","stream":"stdout","cursor":"0","next":"12","eof":false,"data":"hello\u{1b}\nworld"}),
        ),
        (
            "/api/v1/jobs/job_one/logs?stream=stderr&cursor=12&limit=4096".into(),
            json!({"job":"job_one","stream":"stderr","cursor":"12","next":"12","eof":true,"data":""}),
        ),
    ])
}

async fn exercise(server: &server::Server, input: &str) -> String {
    let defaults = ClientDefaults {
        soul: Some("soul_one".into()),
        strand: Some("ss_one".into()),
    };
    let client = reqwest::Client::new();
    let mut output = Vec::new();
    session(
        Request {
            client: &client,
            base: &server.base,
            defaults: &defaults,
            bearer: None,
            memory: None,
        },
        &mut Cursor::new(input),
        &mut output,
    )
    .await
    .unwrap();
    String::from_utf8(output).unwrap()
}

#[tokio::test]
async fn inspection() {
    let server = server::spawn(routes()).await;
    let output = exercise(&server, "/jobs\n/job 1\n/job 1 stderr 12\n/exit\n").await;
    assert!(output.contains("1: job_one · succeeded · detached work"));
    assert!(!output.contains("job_other"));
    assert!(output.contains("job job_one · succeeded · exit 0"));
    assert!(output.contains("effect: effect_one"));
    assert!(output.contains("hello\nworld"));
    assert!(!output.contains('\u{1b}'));
    assert!(output.contains("continue/refresh: /job 1 stdout 12"));
    assert!(output.contains("stderr · cursor 12 → 12 · current eof true"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 8);
    assert!(requests.iter().all(|held| held.starts_with("GET ")));
    for request in requests
        .iter()
        .filter(|held| held.starts_with("GET /api/v1/jobs"))
    {
        assert!(request.contains("x-santi-soul-id: soul_one\r\n"));
    }
}

#[tokio::test]
async fn selectors() {
    let server = server::spawn(routes()).await;
    let output = exercise(&server, "/job\n/job 1\n/jobs\n/job 0\n/job 2\n/job x\n/job 1 stdin\n/job 1 stdout -1\n/job 1 stdout 0 extra\n/jobs extra\n/exit\n").await;
    assert_eq!(output.matches("job inspection refused:").count(), 9);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests.iter().all(|held| held.starts_with("GET ")));
}

#[tokio::test]
async fn ownership() {
    let mut routes = routes();
    let mut foreign = job("job_one", "ss_one");
    foreign["origin"]["soul"] = json!("soul_foreign");
    routes.insert("/api/v1/jobs".into(), json!([foreign]));
    let server = server::spawn(routes).await;
    let output = exercise(&server, "/jobs\n/job 1\n/exit\n").await;
    assert!(output.contains("job response belongs to another soul"));
    assert!(!output.contains("detached"));
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn identity() {
    let mut routes = routes();
    routes.insert("/api/v1/jobs/job_one".into(), job("job_other", "ss_one"));
    let server = server::spawn(routes).await;
    let output = exercise(&server, "/jobs\n/job 1\n/exit\n").await;
    assert!(output.contains("job response does not match"));
    assert_eq!(server.requests.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn log() {
    let mut routes = routes();
    routes.insert("/api/v1/jobs/job_one/logs?stream=stdout&cursor=0&limit=4096".into(), json!({"job":"job_foreign","stream":"stdout","cursor":"0","next":"4","eof":true,"data":"hidden"}));
    let server = server::spawn(routes).await;
    let output = exercise(&server, "/jobs\n/job 1\n/exit\n").await;
    assert!(output.contains("log response does not match"));
    assert!(!output.contains("hidden"));
}

#[tokio::test]
async fn bounded() {
    let mut routes = routes();
    routes.insert(
        "/api/v1/jobs".into(),
        json!(
            (1..=25)
                .map(|n| job(&format!("job_{n}"), "ss_one"))
                .collect::<Vec<_>>()
        ),
    );
    let server = server::spawn(routes).await;
    let output = exercise(&server, "/jobs\n/job 25\n/exit\n").await;
    assert!(output.contains("24: job_24"));
    assert!(!output.contains("25: job_25"));
    assert!(output.contains("1 further jobs omitted"));
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn oversized() {
    let mut routes = routes();
    routes.insert("/api/v1/jobs/job_one/logs?stream=stdout&cursor=0&limit=4096".into(), json!({"job":"job_one","stream":"stdout","cursor":"0","next":"4097","eof":false,"data":"hidden"}));
    let server = server::spawn(routes).await;
    let output = exercise(&server, "/jobs\n/job 1\n/exit\n").await;
    assert!(output.contains("log response exceeds"));
    assert!(!output.contains("hidden"));
}

#[tokio::test]
async fn deadline() {
    let mut routes = routes();
    routes.insert("/api/v1/jobs".into(), json!("stall"));
    let server = server::spawn(routes).await;
    let start = std::time::Instant::now();
    let output = exercise(&server, "/jobs\n/exit\n").await;
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(
        output.contains("job inspection timed out") || output.contains("job inspection refused")
    );
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}
