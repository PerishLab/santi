use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SOUL: &str = "soul_query";

async fn serve(body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("address"));
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut request = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let count = stream.read(&mut chunk).await.expect("read");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            let text = String::from_utf8_lossy(&request);
            let Some((head, rest)) = text.split_once("\r\n\r\n") else {
                continue;
            };
            let expected = head
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            if rest.len() >= expected {
                break;
            }
        }
        stream
            .write_all(response.as_bytes())
            .await
            .expect("respond");
        String::from_utf8_lossy(&request).to_string()
    });
    (base, server)
}

fn command(base: &str, strand: Option<&str>, args: &[&str]) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"));
    command
        .arg("--base-url")
        .arg(base)
        .args(args)
        .env("SANTI_SOUL_ID", SOUL)
        .env_remove("SANTI_STRAND_ID");
    if let Some(strand) = strand {
        command.env("SANTI_STRAND_ID", strand);
    }
    command
}

fn body(request: &str) -> serde_json::Value {
    let (_, body) = request.split_once("\r\n\r\n").expect("request body");
    serde_json::from_str(body).expect("json body")
}

#[tokio::test]
async fn ambient() {
    let (base, server) = serve(r#"{"count":0}"#).await;
    let output = command(
        &base,
        Some("ambient"),
        &["job", "query", "--all-strands", "--count"],
    )
    .output()
    .await
    .expect("client");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = server.await.expect("server");
    assert!(request.starts_with("POST /api/v1/query HTTP/1.1\r\n"));
    assert!(request.to_ascii_lowercase().contains(&format!(
        "x-santi-soul-id: {soul}\r\n",
        soul = SOUL.to_ascii_lowercase()
    )));
    let body = body(&request);
    assert_eq!(body["all_strands"], true);
    assert_eq!(body["count"], true);
    assert!(body.get("strand").is_none());
}

#[tokio::test]
async fn positions() {
    for args in [
        vec!["--strand", "chosen", "job", "query", "--count"],
        vec!["job", "query", "--strand", "chosen", "--count"],
    ] {
        let (base, server) = serve(r#"{"count":0}"#).await;
        let output = command(&base, Some("ambient"), &args)
            .output()
            .await
            .expect("client");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let body = body(&server.await.expect("server"));
        assert_eq!(body["strand"], "chosen", "args {args:?}");
        assert_eq!(body["all_strands"], false);
    }
}

#[tokio::test]
async fn missing() {
    let output = command("http://127.0.0.1:1", None, &["job", "query", "--count"])
        .output()
        .await
        .expect("client");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no strand context"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn conflict() {
    for args in [
        vec!["--strand", "chosen", "job", "query", "--all-strands"],
        vec!["job", "query", "--strand", "chosen", "--all-strands"],
    ] {
        let output = command("http://127.0.0.1:1", Some("ambient"), &args)
            .output()
            .await
            .expect("client");
        assert!(!output.status.success(), "args {args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("conflicts"),
            "args {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[tokio::test]
async fn unfiltered() {
    let (base, server) = serve(r#"{"rows":[]}"#).await;
    let output = command(
        &base,
        Some("ambient"),
        &["query", "from Job where state = \"running\" limit 5"],
    )
    .output()
    .await
    .expect("client");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = server.await.expect("server");
    assert!(request.starts_with("POST /api/v1/query HTTP/1.1\r\n"));
    let body = body(&request);
    assert_eq!(body["q"], "from Job where state = \"running\" limit 5");
    assert!(body.get("strand").is_none());
    assert!(body.get("all_strands").is_none());
}

#[tokio::test]
async fn empty() {
    let output = command("http://127.0.0.1:1", Some(""), &["job", "query", "--count"])
        .output()
        .await
        .expect("client");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("SANTI_STRAND_ID is empty"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn all() {
    let (base, server) = serve(r#"{"count":0}"#).await;
    let output = command(
        &base,
        Some(""),
        &["job", "query", "--all-strands", "--count"],
    )
    .output()
    .await
    .expect("client");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = body(&server.await.expect("server"));
    assert_eq!(body["all_strands"], true);
    assert!(body.get("strand").is_none());
}
