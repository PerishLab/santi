use super::support;
use clap::Parser;
use santi::cli::Cli;
use serde_json::{Value, json};

#[test]
fn modes() {
    let base = ["santi", "inference", "sources", "import", "catalog.json"];
    for flags in [vec![], vec!["--dry-run", "--apply"]] {
        assert!(Cli::try_parse_from(base.into_iter().chain(flags)).is_err());
    }
    for flag in ["--apply", "--dry-run"] {
        Cli::try_parse_from(base.into_iter().chain([flag])).unwrap();
    }
}

#[tokio::test]
async fn preview() {
    let home = support::home();
    let wire = support::serve("200 OK", r#"[{"kind":"provider","name":"demo","id":98}]"#).await;
    let out = support::invoke(
        &wire.base,
        home.path(),
        &[],
        &["inference", "sources", "export"],
    )
    .await;
    assert!(out.status.success());
    let archive: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        archive,
        json!({"version":1,"resources":[{"kind":"provider","name":"demo"}]})
    );
    assert!(
        support::capture(wire)
            .await
            .starts_with("GET /api/v1/inference/sources ")
    );
    std::fs::write(home.path().join("portable.json"), out.stdout).unwrap();
    let wire = support::serve("200 OK", "[]").await;
    let out = support::invoke(
        &wire.base,
        home.path(),
        &[("SANTI_OPERATOR", "missing")],
        &[
            "inference",
            "sources",
            "import",
            "portable.json",
            "--dry-run",
        ],
    )
    .await;
    assert!(out.status.success());
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["create"], 1);
    let raw = support::capture(wire).await;
    assert!(raw.starts_with("GET /api/v1/inference/sources "));
    assert!(!raw.contains("x-santi-catalog-token"));
}

#[tokio::test]
async fn retry() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    let home = support::home();
    let archive = json!({"version":1,"resources":[
        {"kind":"alias","name":"old","access":"route"},
        {"kind":"provider","name":"p"},
        {"kind":"access","name":"route","provider":"p","protocol":"openai_responses",
         "endpoint":"https://example.invalid","credential":"custody://operator"}
    ]});
    std::fs::write(home.path().join("in.json"), archive.to_string()).unwrap();
    std::fs::write(home.path().join("fake-token"), "fixture-only").unwrap();
    let args = ["inference", "sources", "import", "in.json", "--apply"];
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut rows = vec![json!({"kind":"provider","name":"p","id":80})];
        for step in 0..6 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = support::read(&mut stream).await;
            let (status, body) = response(&request, step, &mut rows);
            let body = body.to_string();
            let reply = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let env = [("SANTI_OPERATOR", "fake-token")];
    let failed = support::invoke(&base, home.path(), &env, &args).await;
    support::refused(&failed, "1 confirmed");
    assert!(!String::from_utf8_lossy(&failed.stderr).contains("private-error-body"));
    for matched in [2, 3] {
        let result = support::invoke(&base, home.path(), &env, &args).await;
        support::applied(&result);
        let report: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["matched"], matched);
    }
    server.await.unwrap();
}

fn response(request: &str, step: usize, rows: &mut Vec<Value>) -> (&'static str, Value) {
    let method = if matches!(step, 0 | 3 | 5) {
        "GET"
    } else {
        "POST"
    };
    assert_eq!(
        support::line(request),
        format!("{method} /api/v1/inference/sources HTTP/1.1")
    );
    if method == "GET" {
        assert!(!request.contains("x-santi-catalog-token"));
        ("200 OK", json!(rows))
    } else {
        assert!(request.contains("x-santi-catalog-token: fixture-only"));
        let mut native = support::json(request);
        assert!(native.get("id").is_none());
        if step == 1 {
            assert_eq!(native["kind"], "access");
            assert_eq!(native["provider"], 80);
        } else {
            assert_eq!(native["kind"], "alias");
            assert_eq!(native["access"], 43);
        }
        if step == 2 {
            ("409 Conflict", json!({"detail":"private-error-body"}))
        } else {
            native["id"] = json!(if step == 1 { 43 } else { 55 });
            rows.push(native.clone());
            ("200 OK", native)
        }
    }
}
