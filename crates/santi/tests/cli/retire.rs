use clap::Parser;
use santi::cli::{Cli, Command, StrandCommand};
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
    let Command::Strand(StrandCommand::End { id }) = parsed.command else {
        panic!("end command");
    };
    assert_eq!(id, "target");
}

#[tokio::test]
async fn transport() {
    for status in ["204 No Content", "404 Not Found", "409 Conflict"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let response =
            format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
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
                "end",
                "target",
            ])
            .env("SANTI_STRAND_ID", "environment")
            .output()
            .await
            .expect("client");
        assert_eq!(
            output.status.success(),
            status.starts_with("204"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            server
                .await
                .expect("server")
                .starts_with("DELETE /api/v1/strands/target HTTP/1.1\r\n")
        );
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
