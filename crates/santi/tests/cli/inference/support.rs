use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

pub struct Wire {
    pub base: String,
    pub request: oneshot::Receiver<String>,
}

pub async fn serve(status: &str, body: &str) -> Wire {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("address");
    let status = status.to_string();
    let body = body.to_string();
    let (send, request) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let raw = read(&mut stream).await;
        send.send(raw).expect("record request");
        respond(&mut stream, &status, &body).await;
    });
    Wire {
        base: format!("http://{address}"),
        request,
    }
}

pub async fn read(stream: &mut TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let boundary = loop {
        let count = stream.read(&mut chunk).await.expect("read request");
        assert_ne!(count, 0, "request ended before headers");
        buffer.extend_from_slice(&chunk[..count]);
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
        let count = stream.read(&mut chunk).await.expect("read body");
        assert_ne!(count, 0, "request ended before body");
        buffer.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8_lossy(&buffer).to_string()
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("respond");
}

pub async fn capture(wire: Wire) -> String {
    tokio::time::timeout(Duration::from_secs(3), wire.request)
        .await
        .expect("bounded request capture")
        .expect("request captured")
}

pub fn line(raw: &str) -> &str {
    raw.lines().next().unwrap_or_default()
}

pub fn body(raw: &str) -> &str {
    raw.split_once("\r\n\r\n").map_or("", |(_, body)| body)
}

pub fn json(raw: &str) -> serde_json::Value {
    serde_json::from_str(body(raw)).expect("json body")
}

pub fn home() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}

pub async fn invoke(base: &str, home: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"));
    command
        .current_dir(home)
        .env("SANTI_HOME", home)
        .env_remove("SANTI_BASE_URL")
        .env_remove("SANTI_API_KEY")
        .env_remove("SANTI_AUTH_TOKEN_URL")
        .env_remove("SANTI_AUTH_CLIENT_ID")
        .env_remove("SANTI_AUTH_USERNAME")
        .env_remove("SANTI_AUTH_PASSWORD")
        .env_remove("SANTI_TOKEN_CACHE")
        .env_remove("SANTI_STRAND_ID")
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_INFERENCE_PROVIDER")
        .env_remove("SANTI_INFERENCE_MODEL")
        .env_remove("SANTI_INFERENCE_REASONING_EFFORT");
    for (key, value) in env {
        command.env(key, value);
    }
    command
        .args(["--base-url", base])
        .args(args)
        .output()
        .await
        .expect("run santi")
}

pub fn file(home: &Path, text: &str) -> PathBuf {
    let path = home.join("client.toml");
    std::fs::write(&path, text).expect("write client file");
    path
}

pub const READ: &str = r#"{"declaration":{"provider":"p","model":"m","reasoning_effort":null},"effective":{"provider":"p","model":"m","source":{"provider":"strand","model":"strand"}},"source":{"provider":"strand","model":"strand"}}"#;

pub async fn call(
    wire: Wire,
    home: &Path,
    env: &[(&str, &str)],
    args: &[&str],
) -> (Output, String) {
    let output = invoke(&wire.base, home, env, args).await;
    let request = capture(wire).await;
    (output, request)
}

pub fn applied(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn refused(output: &Output, needle: &str) {
    assert!(!output.status.success(), "expected failure for {needle}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(needle),
        "stderr {stderr:?} does not mention {needle:?}"
    );
}
