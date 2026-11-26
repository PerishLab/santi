use clap::Parser;
use santi::cli::{Cli, Command, Strand};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn explicit() {
    for args in [
        vec!["santi", "strand", "end"],
        vec!["santi", "--strand", "ambient", "strand", "end"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    let parsed = Cli::try_parse_from(["santi", "--strand", "ambient", "strand", "end", "target"])
        .expect("explicit target");
    let Command::Strand(Strand::End { id }) = parsed.command else {
        panic!("end command");
    };
    assert_eq!(id, "target");
}

#[tokio::test]
async fn transport() {
    let record =
        r#"{"id":"target","ended":"2026-11-26T00:00:00Z","jobs":["job"],"messages":2,"turns":1}"#;
    for (command, request, status) in [
        ("end", "DELETE /api/v1/strands/target HTTP/1.1", "200 OK"),
        (
            "end",
            "DELETE /api/v1/strands/target HTTP/1.1",
            "409 Conflict",
        ),
        (
            "retirement",
            "GET /api/v1/strands/target/retirement HTTP/1.1",
            "200 OK",
        ),
        (
            "retirement",
            "GET /api/v1/strands/target/retirement HTTP/1.1",
            "404 Not Found",
        ),
    ] {
        let accepted = status.starts_with("200");
        let body = if accepted { record } else { "" };
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request");
            let mut bytes = [0; 4096];
            let count = stream.read(&mut bytes).await.expect("read");
            let request = String::from_utf8_lossy(&bytes[..count]).to_string();
            stream
                .write_all(response.as_bytes())
                .await
                .expect("respond");
            request
        });
        let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
            .args([
                "--base-url",
                &base,
                "--strand",
                "ambient",
                "strand",
                command,
                "target",
            ])
            .env("SANTI_STRAND_ID", "environment")
            .output()
            .await
            .expect("client");
        assert_eq!(
            output.status.success(),
            accepted,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if output.status.success() {
            let value: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("evidence");
            assert_eq!(
                value,
                serde_json::from_str::<serde_json::Value>(record).unwrap()
            );
        }
        assert!(server.await.expect("server").starts_with(request));
    }
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
        .args(["strand", "end"])
        .env("SANTI_STRAND_ID", "environment")
        .output()
        .await
        .expect("client");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("<ID>"));
}
