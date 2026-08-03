use std::process::Output;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

struct Server {
    base: String,
    request: oneshot::Receiver<String>,
}

#[derive(Clone, Copy)]
enum Loss {
    Transport,
    Body,
    Malformed,
    Redirect,
}

struct Ambiguity {
    base: String,
    committed: Arc<AtomicUsize>,
    redirected: Arc<AtomicUsize>,
}

async fn serve(status: &str, body: &str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake server");
    let address = listener.local_addr().expect("fake server address");
    let status = status.to_string();
    let body = body.to_string();
    let (send, request) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept request");
        let mut buffer = vec![0_u8; 4096];
        let read = stream.read(&mut buffer).await.expect("read request");
        send.send(String::from_utf8_lossy(&buffer[..read]).to_string())
            .expect("record request");
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write response");
    });
    Server {
        base: format!("http://{address}"),
        request,
    }
}
async fn invoke(base: &str, soul: Option<&str>) -> Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"));
    command
        .args(["--base-url", base])
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID");
    if let Some(soul) = soul {
        command.args(["--soul", soul]);
    }
    command
        .args(["strand", "create"])
        .output()
        .await
        .expect("run santi")
}
async fn ambiguous(loss: Loss, strand: bool) -> Ambiguity {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake server");
    let address = listener.local_addr().expect("fake server address");
    let target = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind redirect target");
    let location = target.local_addr().expect("redirect target address");
    let committed = Arc::new(AtomicUsize::new(0));
    let redirected = Arc::new(AtomicUsize::new(0));
    let recorded = committed.clone();
    let replayed = redirected.clone();
    tokio::spawn(async move {
        let (mut stream, _) = target.accept().await.expect("accept redirect replay");
        read(&mut stream).await;
        replayed.fetch_add(1, Ordering::SeqCst);
        respond(&mut stream, "not-json").await;
    });
    tokio::spawn(async move {
        if strand {
            let (mut stream, _) = listener.accept().await.expect("accept soul query");
            read(&mut stream).await;
            respond(&mut stream, r#"{"id":"soul_jarvis"}"#).await;
        }
        let (mut stream, _) = listener.accept().await.expect("accept creation");
        read(&mut stream).await;
        recorded.fetch_add(1, Ordering::SeqCst);
        match loss {
            Loss::Transport => {}
            Loss::Body => {
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\nconnection: close\r\n\r\n{",
                    )
                    .await
                    .expect("partial success");
            }
            Loss::Malformed => respond(&mut stream, "not-json").await,
            Loss::Redirect => {
                let response = format!(
                    "HTTP/1.1 307 Temporary Redirect\r\nlocation: http://{location}/replayed\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("redirect response");
            }
        }
    });
    Ambiguity {
        base: format!("http://{address}"),
        committed,
        redirected,
    }
}
async fn read(stream: &mut TcpStream) {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let boundary = loop {
        let read = stream.read(&mut chunk).await.expect("read request");
        assert_ne!(read, 0, "request ended before headers");
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(boundary) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break boundary + 4;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..boundary]);
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len() < boundary + length {
        let read = stream.read(&mut chunk).await.expect("read request body");
        assert_ne!(read, 0, "request ended before body");
        buffer.extend_from_slice(&chunk[..read]);
    }
}
async fn respond(stream: &mut TcpStream, body: &str) {
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("write response");
}
async fn identify(base: &str, soul: Option<&str>) -> Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"));
    command
        .args(["--base-url", base])
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID");
    if let Some(soul) = soul {
        command.args(["--soul", soul]);
    }
    command.args(["tui"]).output().await.expect("run tui")
}
#[tokio::test]
async fn owner() {
    let server = serve(
        "200 OK",
        r#"{"strand":{"id":"ss_secretary","soul":"soul_jarvis"}}"#,
    )
    .await;
    let output = invoke(&server.base, Some("soul_jarvis")).await;
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("ss_secretary"));
    let request = server.request.await.expect("read request");
    assert!(request.starts_with("POST /api/v1/souls/soul_jarvis/strands HTTP/1.1"));

    let server = serve(
        "200 OK",
        r#"{"strand":{"id":"ss_default","soul":"soul_default"}}"#,
    )
    .await;
    let output = invoke(&server.base, None).await;
    assert!(output.status.success());
    let request = server.request.await.expect("read request");
    assert!(request.starts_with("POST /api/v1/strands HTTP/1.1"));
}
#[tokio::test]
async fn outcomes() {
    let server = serve("404 Not Found", r#"{"error":"soul not found"}"#).await;
    let output = invoke(&server.base, Some("soul_missing")).await;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=not_created"));

    let server = serve(
        "500 Internal Server Error",
        r#"{"error":"readback failed"}"#,
    )
    .await;
    let output = invoke(&server.base, Some("soul_jarvis")).await;
    let error = String::from_utf8_lossy(&output.stderr);
    let request = server.request.await.expect("committed request");
    assert!(!output.status.success());
    assert!(request.starts_with("POST /api/v1/souls/soul_jarvis/strands HTTP/1.1"));
    assert!(error.contains("outcome=state_unknown"));
    assert!(error.contains("server-assigned identity is unknown"));
    assert!(error.contains("SANTI_SOUL_ID=soul_jarvis"));
    assert!(error.contains("do not retry"));
    assert!(error.contains("santi strand list"));

    let server = serve("200 OK", "not-json").await;
    let output = invoke(&server.base, Some("soul_jarvis")).await;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=state_unknown"));

    let server = serve(
        "200 OK",
        r#"{"strand":{"id":"ss_wrong","soul":"soul_default"}}"#,
    )
    .await;
    let output = invoke(&server.base, Some("soul_jarvis")).await;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=applied_mismatch"));
}
#[tokio::test]
async fn ambiguity() {
    for loss in [Loss::Transport, Loss::Body, Loss::Malformed] {
        let server = ambiguous(loss, false).await;
        let output = invoke(&server.base, Some("soul_jarvis")).await;
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(error.contains("strand create outcome=state_unknown"));
        assert!(error.contains("server-assigned identity is unknown"));
        assert!(error.contains("SANTI_SOUL_ID=soul_jarvis"));
        assert!(error.contains("do not retry"));
        assert!(error.contains("santi strand list"));
        assert!(!error.contains("outcome=not_created"));
        assert!(!error.contains("SANTI_STRAND_ID=ss_"));
        assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn soul() {
    for loss in [Loss::Transport, Loss::Body, Loss::Malformed] {
        let server = ambiguous(loss, false).await;
        let output = identify(&server.base, None).await;
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(error.contains("tui soul creation outcome=state_unknown"));
        assert!(error.contains("server-assigned identity is unknown"));
        assert!(error.contains("do not retry"));
        assert!(error.contains("GET /api/v1/souls"));
        assert!(error.contains("without --memory-file"));
        assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn strand() {
    for loss in [Loss::Transport, Loss::Body, Loss::Malformed] {
        let server = ambiguous(loss, true).await;
        let output = identify(&server.base, Some("soul_jarvis")).await;
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(error.contains("tui strand creation outcome=state_unknown"));
        assert!(error.contains("known soul soul_jarvis remains recoverable"));
        assert!(error.contains("SANTI_SOUL_ID=soul_jarvis"));
        assert!(error.contains("do not retry"));
        assert!(error.contains("santi strand list"));
        assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn redirects() {
    let server = ambiguous(Loss::Redirect, false).await;
    let output = invoke(&server.base, Some("soul_jarvis")).await;
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=state_unknown"));
    assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    assert_eq!(server.redirected.load(Ordering::SeqCst), 0);

    let server = ambiguous(Loss::Redirect, false).await;
    let output = identify(&server.base, None).await;
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=state_unknown"));
    assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    assert_eq!(server.redirected.load(Ordering::SeqCst), 0);

    let server = ambiguous(Loss::Redirect, true).await;
    let output = identify(&server.base, Some("soul_jarvis")).await;
    assert!(String::from_utf8_lossy(&output.stderr).contains("outcome=state_unknown"));
    assert_eq!(server.committed.load(Ordering::SeqCst), 1);
    assert_eq!(server.redirected.load(Ordering::SeqCst), 0);
}
