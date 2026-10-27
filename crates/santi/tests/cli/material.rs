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
            (
                "GET",
                "/api/v1/strands/ss_default/materials?available=true&effective=false&offset=0&limit=20",
                None,
            ),
            (
                "GET",
                "/api/v1/souls/soul_default/materials?available=false&effective=false&offset=0&limit=20&reference=perish%3Aplumb&blob=SKILL.md",
                None,
            ),
            ("GET", "/api/v1/souls/soul_explicit", None),
            (
                "POST",
                "/api/v1/souls/soul_default/materials/load",
                Some("load"),
            ),
            (
                "POST",
                "/api/v1/strands/ss_explicit/materials/drop",
                Some("drop"),
            ),
            (
                "GET",
                "/api/v1/strands/ss_default/materials?available=false&effective=true&offset=0&limit=20&operation=load",
                None,
            ),
            (
                "GET",
                "/api/v1/souls/soul_explicit/materials?available=false&effective=false&offset=0&limit=20",
                None,
            ),
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
        vec!["strand", "material", "query", "--available"],
        vec![
            "soul",
            "material",
            "query",
            "perish:plumb",
            "--blob",
            "SKILL.md",
        ],
        vec!["soul", "soul_explicit"],
        vec![
            "soul",
            "material",
            "load",
            "perish:plumb",
            "--operation",
            "load",
        ],
        vec![
            "strand",
            "material",
            "drop",
            "perish:plumb",
            "--id",
            "ss_explicit",
            "--operation",
            "drop",
        ],
        vec![
            "strand",
            "material",
            "query",
            "--effective",
            "--operation",
            "load",
        ],
        vec!["soul", "material", "query", "--id", "soul_explicit"],
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
                "--soul",
                "soul_default",
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
    let expected = if matches!(kind, "load" | "drop") {
        serde_json::json!({"reference":"perish:plumb","operation":kind})
    } else {
        serde_json::json!({"kind":kind})
    };
    assert_eq!(body, expected);
}
