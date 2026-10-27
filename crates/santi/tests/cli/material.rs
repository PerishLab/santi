use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn transport() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for (method, path, kind) in [
            (
                "POST",
                "/api/v1/strands/ss_default/materials",
                Some("skill"),
            ),
            (
                "POST",
                "/api/v1/strands/ss_explicit/materials",
                Some("system_prompt"),
            ),
            ("GET", "/api/v1/openapi.json", None),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = request(&mut stream).await;
            assert!(request.starts_with(&format!("{method} {path} ")));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer fixture-token")
            );
            check(&request, kind);
            let body = r#"{"text":"operating material"}"#;
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
    });
    let home = tempfile::tempdir().unwrap();
    for args in [
        vec!["strand", "material"],
        vec![
            "strand",
            "material",
            "ss_explicit",
            "--kind",
            "system_prompt",
        ],
        vec!["schema"],
    ] {
        let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
            .current_dir(home.path())
            .env("SANTI_HOME", home.path())
            .args([
                "--base-url",
                &base,
                "--api-key",
                "fixture-token",
                "--auth-token-url",
                "",
                "--auth-client-id",
                "",
                "--auth-username",
                "",
                "--auth-password",
                "",
                "--strand",
                "ss_default",
            ])
            .args(args)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["text"],
            "operating material"
        );
    }
    server.await.unwrap();
}

async fn request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0u8; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0 && bytes.len() < 8192);
        bytes.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&bytes);
        if let Some((headers, body)) = text.split_once("\r\n\r\n") {
            let length = length(headers);
            if body.len() >= length {
                return text.into_owned();
            }
        }
    }
}

fn length(headers: &str) -> usize {
    let headers = headers.to_ascii_lowercase();
    headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .unwrap_or("0")
        .parse()
        .unwrap()
}

fn check(request: &str, kind: Option<&str>) {
    let Some(kind) = kind else { return };
    let body: serde_json::Value =
        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, serde_json::json!({"kind":kind}));
}
