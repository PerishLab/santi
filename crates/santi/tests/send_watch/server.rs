use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};
use std::time::Duration;

use santi::cli::WatchFormat;
use santi::client::{Request, Target, emit};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Notify,
};

use super::Wire;

#[derive(Clone, Copy)]
pub(crate) enum Scenario {
    Fast,
    Queued,
    Failed,
    Closed,
    Status,
    Post,
    Rejected,
    Unreadable,
    Broken,
    Malformed,
    Missing,
    Held,
    Paused,
    Limit,
    Abandoned,
    Warning,
    Without,
    Invalid,
    Foreign,
    Inbox,
    Strand,
}

pub(crate) struct Server {
    pub(crate) base: String,
    pub(crate) counts: Counts,
}

#[derive(Clone)]
pub(crate) struct Counts {
    pub(crate) post: Arc<AtomicUsize>,
    pub(crate) events: Arc<AtomicUsize>,
    pub(crate) receipts: Arc<AtomicUsize>,
    pub(crate) observed: Arc<AtomicBool>,
    pub(crate) queued: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    posted: Arc<Notify>,
    pub(crate) armed: Arc<Notify>,
    terminal: Arc<Notify>,
}

pub(crate) async fn spawn(scenario: Scenario) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let counts = Counts {
        post: Arc::new(AtomicUsize::new(0)),
        events: Arc::new(AtomicUsize::new(0)),
        receipts: Arc::new(AtomicUsize::new(0)),
        observed: Arc::new(AtomicBool::new(false)),
        queued: Arc::new(AtomicBool::new(false)),
        state: Arc::new(AtomicU8::new(4)),
        posted: Arc::new(Notify::new()),
        armed: Arc::new(Notify::new()),
        terminal: Arc::new(Notify::new()),
    };
    let held = counts.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let counts = held.clone();
            tokio::spawn(async move { handle(stream, counts, scenario).await });
        }
    });
    Server {
        base: format!("http://{address}"),
        counts,
    }
}

pub(crate) async fn invoke(base: &str) -> (anyhow::Result<()>, Vec<u8>) {
    let client = reqwest::Client::new();
    let mut output = Vec::new();
    let result = emit(
        Request {
            target: Target::new(&client, base, "ss_cli", WatchFormat::Raw),
            body: serde_json::json!({"content":[{"type":"text","text":"hello"}]}),
            watch: true,
        },
        &mut output,
    )
    .await;
    (result, output)
}

impl Server {
    pub(crate) async fn armed(&self) {
        while self.counts.post.load(Ordering::SeqCst) == 0 {
            let posted = self.counts.posted.notified();
            if self.counts.post.load(Ordering::SeqCst) == 0 {
                posted.await;
            }
        }
        while self.counts.state.load(Ordering::SeqCst) == 4 {
            let armed = self.counts.armed.notified();
            if self.counts.state.load(Ordering::SeqCst) == 4 {
                armed.await;
            }
        }
        assert!(self.counts.events.load(Ordering::SeqCst) > 0);
    }
}

async fn handle(stream: TcpStream, counts: Counts, scenario: Scenario) {
    let mut wire = Wire(stream);
    let Some((method, path)) = wire.read().await else {
        return;
    };
    match (method.as_str(), path.as_str()) {
        ("POST", "/api/v1/strands/ss_cli/send") => {
            counts.post.fetch_add(1, Ordering::SeqCst);
            counts
                .observed
                .store(counts.events.load(Ordering::SeqCst) > 0, Ordering::SeqCst);
            if matches!(scenario, Scenario::Post) {
                counts.posted.notify_waiters();
                wire.respond("503 Service Unavailable", r#"{"error":"after commit"}"#)
                    .await;
                return;
            }
            if matches!(scenario, Scenario::Rejected) {
                counts.posted.notify_waiters();
                wire.respond("422 Unprocessable Entity", r#"{"error":"invalid content"}"#)
                    .await;
                return;
            }
            if matches!(scenario, Scenario::Unreadable) {
                counts.posted.notify_waiters();
                wire.unreadable("422 Unprocessable Entity").await;
                return;
            }
            if matches!(scenario, Scenario::Fast) {
                let terminal = counts.terminal.notified();
                counts.posted.notify_waiters();
                terminal.await;
            } else {
                counts.posted.notify_waiters();
            }
            wire.respond("200 OK", body(scenario)).await;
        }
        ("GET", "/api/v1/strands/ss_cli/events") => {
            counts.events.fetch_add(1, Ordering::SeqCst);
            wire.events(counts, scenario).await;
        }
        ("GET", "/api/v1/receipts/inbox_cli") => {
            counts.receipts.fetch_add(1, Ordering::SeqCst);
            wire.receipt(counts.state.load(Ordering::SeqCst), scenario)
                .await;
            counts.armed.notify_waiters();
        }
        _ => wire.respond("404 Not Found", "not found").await,
    }
}

fn body(scenario: Scenario) -> &'static str {
    match scenario {
        Scenario::Malformed => "not-json",
        Scenario::Missing => r#"{"turn":{"id":"turn_seed"}}"#,
        Scenario::Warning => {
            r#"{"receipt":{"strand":"ss_cli","inbox":"inbox_cli","warning":{"code":"runtime.strand.drive_failed","context":{"recovery":{"command":"santi strand drive ss_cli"}}}},"turn":null}"#
        }
        Scenario::Without => {
            r#"{"receipt":{"warning":{"code":"runtime.strand.drive_failed","context":{"recovery":{"command":"santi strand drive ss_cli"}}}},"turn":null}"#
        }
        Scenario::Invalid => {
            r#"{"receipt":{"strand":"ss_cli","inbox":" inbox_cli","warning":{"code":"runtime.strand.drive_failed","context":{"recovery":{"command":"santi strand drive ss_cli"}}}},"turn":null}"#
        }
        Scenario::Foreign => {
            r#"{"receipt":{"strand":"ss_other","inbox":"inbox_cli","warning":{"code":"runtime.strand.drive_failed","context":{"recovery":{"command":"santi strand drive ss_cli"}}}},"turn":null}"#
        }
        _ => {
            r#"{"receipt":{"strand":"ss_cli","inbox":"inbox_cli","warning":null},"turn":{"id":"turn_seed"}}"#
        }
    }
}

impl Wire {
    async fn events(&mut self, counts: Counts, scenario: Scenario) {
        if matches!(scenario, Scenario::Status) {
            self.respond("500 Internal Server Error", "boom").await;
            return;
        }
        let headers = if matches!(scenario, Scenario::Broken) {
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n"
                .as_slice()
        } else {
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n"
                .as_slice()
        };
        self.0.write_all(headers).await.expect("stream headers");
        if matches!(
            scenario,
            Scenario::Closed
                | Scenario::Malformed
                | Scenario::Missing
                | Scenario::Warning
                | Scenario::Without
                | Scenario::Invalid
                | Scenario::Foreign
        ) {
            return;
        }
        let posted = counts.posted.notified();
        if counts.post.load(Ordering::SeqCst) == 0 {
            posted.await;
        }
        if matches!(scenario, Scenario::Broken) {
            self.0
                .write_all(b"not-a-chunk\r\n")
                .await
                .expect("invalid stream chunk");
            return;
        }
        match scenario {
            Scenario::Held => counts.state.store(0, Ordering::SeqCst),
            Scenario::Paused => counts.state.store(3, Ordering::SeqCst),
            Scenario::Limit => {
                counts.state.store(0, Ordering::SeqCst);
                self.turn("active").await;
                counts.armed.notify_waiters();
                let until = tokio::time::Instant::now() + Duration::from_secs(10 * 60 - 30);
                while tokio::time::Instant::now() < until {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    self.turn("active").await;
                }
                std::future::pending::<()>().await;
            }
            Scenario::Abandoned => counts.state.store(1, Ordering::SeqCst),
            Scenario::Failed => {
                counts.state.store(1, Ordering::SeqCst);
                counts.armed.notify_waiters();
                self.turn("failed").await;
                counts.terminal.notify_waiters();
                return;
            }
            _ => {
                counts.state.store(2, Ordering::SeqCst);
                counts
                    .queued
                    .store(matches!(scenario, Scenario::Queued), Ordering::SeqCst);
                self.turn("completed").await;
                counts.terminal.notify_waiters();
                if !matches!(scenario, Scenario::Queued) {
                    return;
                }
            }
        }
        counts.armed.notify_waiters();
        std::future::pending::<()>().await;
    }

    async fn receipt(&mut self, state: u8, scenario: Scenario) {
        let state = match state {
            1 => "failed",
            2 => "completed",
            3 => "recovered",
            _ => "accepted",
        };
        let inbox = if matches!(scenario, Scenario::Inbox) {
            "inbox_other"
        } else {
            "inbox_cli"
        };
        let strand = if matches!(scenario, Scenario::Strand) {
            "ss_other"
        } else {
            "ss_cli"
        };
        let body = format!(r#"{{"inbox":"{inbox}","strand":"{strand}","state":"{state}"}}"#);
        self.respond("200 OK", &body).await;
    }

    async fn turn(&mut self, beat: &str) {
        let frame = format!(
            "event: turn\ndata: {{\"payload\":{{\"type\":\"turn\",\"beat\":\"{beat}\",\"turn\":\"turn_seed\"}}}}\n\n"
        );
        self.0
            .write_all(frame.as_bytes())
            .await
            .expect("turn frame");
    }
}
