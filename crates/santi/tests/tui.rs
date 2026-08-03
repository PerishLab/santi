use std::io::Cursor;
use std::sync::{Arc, Mutex};

use santi::cli::ClientDefaults;
use santi::client::tui::{Request, session};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct CountingHttpServer {
    base_url: String,
    requests: Arc<Mutex<Vec<(String, String, String)>>>,
}
async fn spawn_server() -> CountingHttpServer {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake server");
    let addr = listener.local_addr().expect("fake server addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let requests = recorded.clone();
            tokio::spawn(async move { handle_request(stream, requests).await });
        }
    });
    CountingHttpServer {
        base_url: format!("http://{addr}"),
        requests,
    }
}
async fn handle_request(
    mut stream: TcpStream,
    requests: Arc<Mutex<Vec<(String, String, String)>>>,
) {
    let Some((method, path, body)) = read(&mut stream).await else {
        return;
    };
    requests
        .lock()
        .unwrap()
        .push((method.clone(), path.clone(), body));
    match (method.as_str(), path.as_str()) {
        ("POST", "/api/v1/souls") => {
            json(&mut stream, r#"{"id":"soul_jarvis"}"#).await;
        }
        ("GET", "/api/v1/souls/soul_jarvis") => {
            json(&mut stream, r#"{"id":"soul_jarvis"}"#).await;
        }
        ("GET", "/api/v1/strands/ss_foreign") => {
            json(
                &mut stream,
                r#"{"strand":{"id":"ss_foreign","soul":"soul_other"},"messages":[]}"#,
            )
            .await;
        }
        ("GET", "/api/v1/strands/ss_ownerless") => {
            json(
                &mut stream,
                r#"{"strand":{"id":"ss_ownerless"},"messages":[]}"#,
            )
            .await;
        }
        ("POST", "/api/v1/souls/soul_jarvis/strands") => {
            json(
                &mut stream,
                r#"{"strand":{"id":"ss_jarvis","soul":"soul_jarvis"}}"#,
            )
            .await;
        }
        ("GET", "/api/v1/strands/ss_jarvis/budget") => {
            let status = if requests.lock().unwrap().len() > 3 {
                "503 Service Unavailable"
            } else {
                "200 OK"
            };
            write_response(
                &mut stream,
                status,
                "application/json",
                r#"{"estimate":{"total":1200},"budget":{"bytes":8000}}"#,
            )
            .await;
        }
        ("POST", "/api/v1/strands/ss_jarvis/send") => {
            json(
                &mut stream,
                r#"{"receipt":{"strand":"ss_jarvis","inbox":"inbox_jarvis","warning":null},"turn":{"id":"turn_seed"}}"#,
            )
            .await;
        }
        ("GET", "/api/v1/receipts/inbox_jarvis") => {
            json(
                &mut stream,
                r#"{"inbox":"inbox_jarvis","strand":"ss_jarvis","state":"completed"}"#,
            )
            .await;
        }
        ("GET", "/api/v1/strands/ss_jarvis/events") => {
            write_response(
                &mut stream,
                "200 OK",
                "text/event-stream",
                concat!(
                    "event: message\n",
                    "data: {\"payload\":{\"type\":\"message\",\"beat\":\"delta\",\"message\":\"stream_predispatch\",\"turn\":\"turn_predispatch\",\"role\":\"soul\",\"text\":\"JARVIS_LIVE_PREDISPATCH\"}}\n\n",
                    "event: turn\n",
                    "data: {\"payload\":{\"type\":\"turn\",\"beat\":\"active\",\"activity\":{\"turn\":\"turn_seed\",\"state\":\"calling\"}}}\n\n",
                    "event: tool\n",
                    "data: {\"payload\":{\"type\":\"tool\",\"beat\":\"called\",\"call\":{\"id\":\"call_seed\",\"tool\":\"shell\"}}}\n\n",
                    "event: message\n",
                    "data: {\"payload\":{\"type\":\"message\",\"beat\":\"delta\",\"message\":\"stream_turn_seed\",\"turn\":\"turn_seed\",\"role\":\"soul\",\"text\":\"I am awake.\\u001b\"}}\n\n",
                    "event: message\n",
                    "data: {\"payload\":{\"beat\":\"completed\",\"turn\":\"turn_seed\",\"message\":{\"text\":\"I am awake.\"}}}\n\n",
                    "event: message\n",
                    "data: {\"payload\":{\"beat\":\"completed\",\"turn\":\"turn_missing\",\"message\":{\"text\":\"Recovered speech.\"}}}\n\n",
                    "event: message\n",
                    "data: {\"payload\":{\"beat\":\"delta\",\"turn\":\"turn_gap\",\"text\":\"Gap-safe speech.\"}}\n\n",
                    "event: gap\n",
                    "data: {\"payload\":{\"type\":\"gap\",\"skipped\":2,\"message\":\"event stream gap: 2 events were not delivered\"}}\n\n",
                    "event: message\n",
                    "data: {\"payload\":{\"beat\":\"completed\",\"turn\":\"turn_gap\",\"message\":{\"text\":\"Gap-safe speech.\"}}}\n\n",
                    "event: turn\n",
                    "data: {\"payload\":{\"beat\":\"completed\",\"turn\":\"turn_seed\"}}\n\n"
                ),
            )
            .await;
        }
        _ => write_response(&mut stream, "404 Not Found", "text/plain", "not found").await,
    }
}
async fn read(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let boundary = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(boundary) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break boundary + 4;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..boundary]).to_string();
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while buffer.len() < boundary + length {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let mut parts = head.lines().next()?.split_whitespace();
    Some((
        parts.next()?.to_string(),
        parts.next()?.to_string(),
        String::from_utf8_lossy(&buffer[boundary..]).to_string(),
    ))
}
async fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("write fake response");
}
async fn json(stream: &mut TcpStream, body: &str) {
    write_response(stream, "200 OK", "application/json", body).await;
}
#[tokio::test]
async fn starts() {
    let server = spawn_server().await;
    let temp = tempfile::tempdir().expect("temp dir");
    let memory = temp.path().join("jarvis.md");
    std::fs::write(&memory, "You are Jarvis.").expect("write memory");
    let defaults = ClientDefaults {
        soul: None,
        strand: None,
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("hello\n/exit\n");
    let mut output = Vec::new();
    session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            memory: Some(memory.display().to_string()),
        },
        &mut input,
        &mut output,
    )
    .await
    .expect("cold start");
    let output = String::from_utf8(output).expect("utf8 output");
    assert!(output.contains("soul: soul_jarvis"));
    assert!(output.contains("strand: ss_jarvis"));
    assert!(output.contains("context: 1200/8000 bytes"));
    assert!(output.contains("soul> I am awake."));
    assert_eq!(output.matches("I am awake.").count(), 1);
    assert!(output.contains("assistant completed turn_missing: Recovered speech."));
    assert!(output.contains("event stream gap: 2 events were not delivered"));
    assert_eq!(output.matches("Gap-safe speech.").count(), 2);
    assert!(output.contains("assistant completed turn_gap: Gap-safe speech."));
    let completed = output
        .find("send completed: receipt inbox_jarvis is durably completed; do not resend")
        .expect("completed receipt proof");
    let auxiliary = output
        .find("auxiliary status refresh failed after completed receipt inbox_jarvis; do not resend")
        .expect("auxiliary refresh guidance");
    assert!(completed < auxiliary);
    assert!(!output.contains('\u{1b}'));
    let marker = output.find("JARVIS_LIVE_PREDISPATCH").expect("live marker");
    let calling = output.find("turn turn_seed: calling").expect("calling");
    let dispatch = output.find("tool call shell").expect("tool dispatch");
    assert!(marker < calling);
    assert!(calling < dispatch);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 7);
    assert!(requests[0].2.contains(r#""memory":"You are Jarvis.""#));
    let send = requests
        .iter()
        .find(|(method, path, _)| method == "POST" && path == "/api/v1/strands/ss_jarvis/send")
        .expect("send request");
    assert!(send.2.contains(r#""text":"hello""#));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.0 == "POST" && request.1.ends_with("/send"))
            .count(),
        1
    );
}
#[tokio::test]
async fn refuses() {
    let server = spawn_server().await;
    let defaults = ClientDefaults {
        soul: Some("soul_jarvis".to_string()),
        strand: Some("ss_foreign".to_string()),
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("/exit\n");
    let mut output = Vec::new();
    let error = session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            memory: None,
        },
        &mut input,
        &mut output,
    )
    .await
    .expect_err("foreign strand must be rejected");
    assert!(error.to_string().contains("belongs to soul soul_other"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/api/v1/strands/ss_foreign");
}
#[tokio::test]
async fn owner() {
    let server = spawn_server().await;
    let defaults = ClientDefaults {
        soul: None,
        strand: Some("ss_ownerless".to_string()),
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("/exit\n");
    let mut output = Vec::new();
    let error = session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            memory: None,
        },
        &mut input,
        &mut output,
    )
    .await
    .expect_err("ownerless strand must be rejected");
    assert!(error.to_string().contains("missing valid /strand/soul"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/api/v1/strands/ss_ownerless");
}
