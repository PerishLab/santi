use santi::watch::{self, Harness};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
#[path = "send_watch/server.rs"]
mod server;
use server::{Scenario, invoke, spawn};
struct Wire(TcpStream);
impl Wire {
    async fn read(&mut self) -> Option<(String, String)> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = self.0.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buffer);
                let mut parts = head.lines().next()?.split_whitespace();
                return Some((parts.next()?.to_string(), parts.next()?.to_string()));
            }
        }
    }
    async fn respond(&mut self, status: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        self.0
            .write_all(response.as_bytes())
            .await
            .expect("response");
    }
    async fn unreadable(&mut self, status: &str) {
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: 100\r\nconnection: close\r\n\r\n{{"
        );
        self.0
            .write_all(response.as_bytes())
            .await
            .expect("truncated response");
    }
}
#[tokio::test]
async fn completion() {
    for scenario in [Scenario::Fast, Scenario::Queued] {
        let server = spawn(scenario).await;
        let (result, _) = invoke(&server.base).await;
        result.expect("durable completion");
        assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
        assert_eq!(server.counts.events.load(Ordering::SeqCst), 1);
        assert!(server.counts.receipts.load(Ordering::SeqCst) >= 1);
        assert!(server.counts.observed.load(Ordering::SeqCst));
        if matches!(scenario, Scenario::Queued) {
            assert!(server.counts.queued.load(Ordering::SeqCst));
        }
    }
}
#[tokio::test]
async fn failure() {
    let server = spawn(Scenario::Failed).await;
    let (result, _) = invoke(&server.base).await;
    let error = result.expect_err("failed receipt");
    assert!(error.to_string().contains("outcome=failed"));
    assert!(error.to_string().contains("santi receipt inbox_cli"));
    assert!(error.to_string().contains("do not resend"));
}
#[tokio::test]
async fn uncertainty() {
    let closed = spawn(Scenario::Closed).await;
    let (result, _) = invoke(&closed.base).await;
    let error = result.expect_err("closed stream");
    assert!(error.to_string().contains("state_unknown"));
    assert!(error.to_string().contains("inbox_cli"));
    assert!(error.to_string().contains("do not resend"));
    let status = spawn(Scenario::Status).await;
    let (result, _) = invoke(&status.base).await;
    let error = result.expect_err("subscription failure").to_string();
    assert!(error.contains("request failed with status"));
    assert_eq!(status.counts.post.load(Ordering::SeqCst), 0);
    for scenario in [Scenario::Inbox, Scenario::Strand] {
        let server = spawn(scenario).await;
        let (result, _) = invoke(&server.base).await;
        let error = result.expect_err("mismatched durable proof");
        assert!(error.to_string().contains("state_unknown"));
        assert!(error.to_string().contains("identity did not match"));
    }
}
#[tokio::test]
async fn post() {
    let server = spawn(Scenario::Post).await;
    let (result, output) = invoke(&server.base).await;
    let error = result.expect_err("readable POST failure").to_string();
    assert!(error.contains("outcome=state_unknown"));
    assert!(error.contains("POST returned status 503"));
    assert!(error.contains("may have been accepted"));
    assert!(error.contains("do not resend"));
    assert!(!error.contains("try again"));
    assert!(output.is_empty());
    assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn rejection() {
    for scenario in [Scenario::Rejected, Scenario::Unreadable] {
        let server = spawn(scenario).await;
        let (result, output) = invoke(&server.base).await;
        let error = result.expect_err("definite POST rejection").to_string();
        assert!(error.contains("outcome=not_accepted"));
        assert!(error.contains("status 422"));
        assert!(!error.contains("state_unknown"));
        assert!(!error.contains("may have been accepted"));
        assert!(!error.contains("do not resend"));
        assert!(output.is_empty());
        assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
        assert_eq!(server.counts.receipts.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn stream() {
    let server = spawn(Scenario::Broken).await;
    let (result, _) = invoke(&server.base).await;
    let error = result.expect_err("stream read failure").to_string();
    assert!(error.contains("outcome=state_unknown"));
    assert!(error.contains("event stream failed"));
    assert!(error.contains("santi receipt inbox_cli"));
    assert!(error.contains("santi strand runtime ss_cli"));
    assert!(error.contains("santi strand drive ss_cli"));
    assert!(error.contains("do not resend"));
    assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
}
#[tokio::test(start_paused = true)]
async fn liveness() {
    use Scenario::*;
    for scenario in [Held, Paused, Abandoned, Limit] {
        let server = spawn(scenario).await;
        let base = server.base.clone();
        let (started, mut start) = tokio::sync::watch::channel(tokio::time::Instant::now());
        let (silence, mut quiet) = tokio::sync::watch::channel(tokio::time::Instant::now());
        let task = tokio::spawn(async move {
            if matches!(scenario, Limit) {
                watch::harness(Harness { started, silence }, invoke(&base)).await
            } else {
                invoke(&base).await
            }
        });
        let target = if matches!(scenario, Limit) {
            start.changed().await.expect("production watch start");
            Some(*start.borrow_and_update() + Duration::from_secs(10 * 60 + 1))
        } else {
            None
        };
        server.armed().await;
        if let Some(target) = target {
            quiet.changed().await.expect("initial silence deadline");
            assert!(*quiet.borrow_and_update() < target);
            tokio::time::sleep_until(target - Duration::from_secs(31)).await;
            while *quiet.borrow_and_update() <= target {
                quiet
                    .changed()
                    .await
                    .expect("pre-limit watch silence deadline");
            }
            let now = tokio::time::Instant::now();
            assert!(now < target);
            tokio::time::advance(target - now).await;
            tokio::time::resume();
        } else {
            tokio::time::advance(Duration::from_secs(61)).await;
        }
        let (result, _) = task.await.expect("watch task");
        assert!(server.counts.receipts.load(Ordering::SeqCst) >= 1);
        let error = result.expect_err("finite non-success").to_string();
        if matches!(scenario, Scenario::Abandoned) {
            assert!(error.contains("outcome=failed"));
        } else {
            assert!(error.contains("state_unknown"));
            assert!(error.contains("santi strand runtime ss_cli"));
            assert!(error.contains("santi strand drive ss_cli"));
        }
        if matches!(scenario, Scenario::Limit) {
            assert!(error.contains("watch reached its ten-minute proof limit"));
            assert!(error.contains("receipt inbox_cli remained accepted"));
        }
        assert!(error.contains("do not resend"));
        assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn identity() {
    for scenario in [Scenario::Malformed, Scenario::Missing] {
        let server = spawn(scenario).await;
        let (result, _) = invoke(&server.base).await;
        let error = result.expect_err("ambiguous identity");
        assert!(error.to_string().contains("state_unknown"));
        assert!(error.to_string().contains("do not resend"));
        assert_eq!(server.counts.post.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn warnings() {
    for scenario in [Scenario::Without, Scenario::Invalid, Scenario::Foreign] {
        let server = spawn(scenario).await;
        let (result, output) = invoke(&server.base).await;
        let error = result.expect_err("invalid warning identity").to_string();
        assert!(error.contains("state_unknown"));
        assert!(error.contains("do not resend"));
        assert!(!error.contains("message was accepted as exact receipt"));
        assert!(output.is_empty());
    }
    let server = spawn(Scenario::Warning).await;
    let (result, output) = invoke(&server.base).await;
    let error = result.expect_err("validated driver warning").to_string();
    assert!(error.contains("exact receipt inbox_cli"));
    assert!(error.contains("do not resend"));
    assert!(error.contains("santi strand drive ss_cli"));
    let output = String::from_utf8(output).expect("warning output");
    assert!(output.contains("inbox_cli"));
}
#[tokio::test]
async fn redirect() {
    let origin = TcpListener::bind("127.0.0.1:0").await.expect("bind origin");
    let target = TcpListener::bind("127.0.0.1:0").await.expect("bind target");
    let base = format!("http://{}", origin.local_addr().expect("origin address"));
    let location = target.local_addr().expect("target address");
    let posts = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(AtomicUsize::new(0));
    let replayed = Arc::new(AtomicUsize::new(0));
    let replay = replayed.clone();
    tokio::spawn(async move {
        let (stream, _) = target.accept().await.expect("accept redirect replay");
        let mut wire = Wire(stream);
        wire.read().await.expect("read redirect replay");
        replay.fetch_add(1, Ordering::SeqCst);
        wire.respond("503 Service Unavailable", "after replay")
            .await;
    });
    let post = posts.clone();
    let event = events.clone();
    tokio::spawn(async move {
        loop {
            let (stream, _) = origin.accept().await.expect("accept origin request");
            let posts = post.clone();
            let events = event.clone();
            tokio::spawn(async move {
                let mut wire = Wire(stream);
                let (method, path) = wire.read().await.expect("read origin request");
                if method == "GET" && path == "/api/v1/strands/ss_cli/events" {
                    events.fetch_add(1, Ordering::SeqCst);
                    wire.0
                        .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n")
                        .await
                        .expect("event headers");
                    std::future::pending::<()>().await;
                } else if method == "POST" && path == "/api/v1/strands/ss_cli/send" {
                    posts.fetch_add(1, Ordering::SeqCst);
                    let response = format!(
                        "HTTP/1.1 307 Temporary Redirect\r\nlocation: http://{location}/replayed\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    );
                    wire.0
                        .write_all(response.as_bytes())
                        .await
                        .expect("redirect");
                }
            });
        }
    });
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
        .args([
            "--base-url",
            &base,
            "--strand",
            "ss_cli",
            "strand",
            "send",
            "--watch",
            "--watch-format",
            "raw",
            "hello",
        ])
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID")
        .output()
        .await
        .expect("run watched send");
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(error.contains("outcome=state_unknown"));
    assert!(!error.contains("outcome=not_accepted"));
    assert_eq!(events.load(Ordering::SeqCst), 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert_eq!(replayed.load(Ordering::SeqCst), 0);
}
