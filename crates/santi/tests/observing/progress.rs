use super::{Wire, invoke};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(receipt: Value, activity: Option<Value>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let held = requests.clone();
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let receipt = receipt.clone();
            let activity = activity.clone();
            let requests = held.clone();
            tokio::spawn(handle(Wire(stream), receipt, activity, requests));
        }
    });
    Server {
        base,
        requests,
        task,
    }
}

async fn handle(
    mut wire: Wire,
    receipt: Value,
    activity: Option<Value>,
    requests: Arc<Mutex<Vec<String>>>,
) {
    let (method, path) = wire.read().await.unwrap();
    requests.lock().unwrap().push(format!("{method} {path}"));
    match path.as_str() {
        "/api/v1/strands/ss_cli/events" => {
            wire.0.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n").await.unwrap();
        }
        "/api/v1/strands/ss_cli/send" => {
            wire.respond(
                "200 OK",
                r#"{"receipt":{"inbox":"inbox_cli","strand":"ss_cli"},"turn":null}"#,
            )
            .await;
        }
        "/api/v1/receipts/inbox_cli" => {
            wire.respond("200 OK", &receipt.to_string()).await;
        }
        "/api/v1/strands/ss_cli/activity" => match activity {
            Some(value) => wire.respond("200 OK", &value.to_string()).await,
            None => std::future::pending::<()>().await,
        },
        _ => wire.respond("404 Not Found", "{}").await,
    }
}

use tokio::io::AsyncWriteExt;

fn receipt() -> Value {
    json!({"inbox":"inbox_cli","strand":"ss_cli","state":"driving",
        "transitions":[{"state":"driving","turn":"turn_current","sequence":2}]})
}

fn activity() -> Value {
    json!({"epoch":"process","revision":8,"strand":"ss_cli",
        "current":{"turn":"turn_current","round":3,"phase":"requesting","call":null}})
}

#[tokio::test]
async fn correlation() {
    let good = activity();
    let mut foreign = good.clone();
    foreign["current"]["turn"] = json!("turn_other");
    let mut empty = good.clone();
    empty["current"] = Value::Null;
    let mut strand = good.clone();
    strand["strand"] = json!("ss_other");
    for (snapshot, matched) in [
        (good, true),
        (foreign, false),
        (empty, false),
        (strand, false),
        (json!({"private":"secret"}), false),
    ] {
        let server = server(receipt(), Some(snapshot)).await;
        let (result, _) = invoke(&server.base).await;
        let error = result.unwrap_err().to_string();
        assert!(error.contains("state_unknown"));
        assert!(error.contains("do not resend"));
        assert!(error.contains("continue observing the same receipt"));
        assert!(!error.contains("explicitly redrive"));
        assert!(!error.contains("secret"));
        assert_eq!(error.contains("phase=requesting, round=3"), matched);
        assert_eq!(error.contains("activity unknown"), !matched);
        assert_eq!(
            server.requests.lock().unwrap().as_slice(),
            [
                "GET /api/v1/strands/ss_cli/events",
                "POST /api/v1/strands/ss_cli/send",
                "GET /api/v1/receipts/inbox_cli",
                "GET /api/v1/strands/ss_cli/activity",
            ]
        );
    }
}

#[tokio::test]
async fn authority() {
    for state in ["completed", "failed", "accepted", "recovered", "driving"] {
        let mut receipt = receipt();
        receipt["state"] = json!(state);
        if state == "driving" {
            receipt["transitions"]
                .as_array_mut()
                .unwrap()
                .push(json!({"state":"recovered","turn":null,"sequence":3}));
        }
        let server = server(receipt, Some(activity())).await;
        let (result, _) = invoke(&server.base).await;
        assert_eq!(result.is_ok(), state == "completed");
        if let Err(error) = result {
            let error = error.to_string();
            assert!(error.contains(if state == "failed" {
                "outcome=failed"
            } else {
                "activity unknown"
            }));
            assert!(!error.contains("phase=requesting"));
        }
        assert_eq!(server.requests.lock().unwrap().len(), 3);
    }
}

#[tokio::test]
async fn bounded() {
    let server = server(receipt(), None).await;
    let start = tokio::time::Instant::now();
    let (result, _) = invoke(&server.base).await;
    assert!(start.elapsed() < std::time::Duration::from_secs(8));
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("snapshot unavailable or invalid")
    );
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn command() {
    let server = server(receipt(), Some(activity())).await;
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
        .args([
            "--base-url",
            &server.base,
            "--strand",
            "ss_cli",
            "strand",
            "activity",
        ])
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID")
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        activity()
    );
    assert_eq!(
        server.requests.lock().unwrap().as_slice(),
        ["GET /api/v1/strands/ss_cli/activity"]
    );
}
