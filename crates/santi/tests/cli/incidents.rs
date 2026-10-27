use clap::Parser;
use santi::cli::{Cli, Command, StrandCommand};

#[test]
fn errors() {
    let parsed = Cli::try_parse_from([
        "santi",
        "errors",
        "--scope-kind",
        "runtime",
        "--scope-id",
        "default",
        "--limit",
        "12",
    ])
    .unwrap();
    let Command::Errors {
        scope_kind,
        scope_id,
        limit,
        ..
    } = parsed.command
    else {
        panic!("expected global errors command");
    };
    assert_eq!(scope_kind, "runtime");
    assert_eq!(scope_id, "default");
    assert_eq!(limit, 12);

    let parsed =
        Cli::try_parse_from(["santi", "strand", "errors", "ss_1", "--limit", "12"]).unwrap();
    let Command::Strand(StrandCommand::Errors { id, limit, .. }) = parsed.command else {
        panic!("expected strand errors command");
    };
    assert_eq!(id.as_deref(), Some("ss_1"));
    assert_eq!(limit, 12);
    assert!(Cli::try_parse_from(["santi", "strand", "rejections", "ss_1"]).is_err());
}

#[test]
fn cursors() {
    let parsed = Cli::try_parse_from([
        "santi",
        "errors",
        "--scope-kind",
        "soul",
        "--scope-id",
        "owner",
        "--before",
        "inc_last",
    ])
    .expect("cursor");
    let Command::Errors { before, .. } = parsed.command else {
        panic!("errors");
    };
    assert_eq!(before.as_deref(), Some("inc_last"));
    let parsed = Cli::try_parse_from([
        "santi", "strand", "errors", "target", "--before", "inc_last",
    ])
    .expect("cursor");
    let Command::Strand(StrandCommand::Errors { before, .. }) = parsed.command else {
        panic!("errors");
    };
    assert_eq!(before.as_deref(), Some("inc_last"));
    assert!(Cli::try_parse_from(["santi", "incident"]).is_err());
    let parsed = Cli::try_parse_from(["santi", "incident", "inc_target"]).expect("incident");
    assert!(matches!(parsed.command, Command::Incident { id } if id == "inc_target"));
}

#[tokio::test]
async fn transport() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for (args, expected) in [
        (
            vec!["incident", "inc_target"],
            "/api/v1/incidents/inc_target",
        ),
        (
            vec![
                "errors",
                "--scope-kind",
                "soul",
                "--scope-id",
                "owner",
                "--limit",
                "3",
                "--before",
                "inc +&?",
            ],
            "/api/v1/errors/soul/owner?limit=3&before=inc+%2B%26%3F",
        ),
        (
            vec![
                "strand", "errors", "target", "--limit", "3", "--before", "inc_last",
            ],
            "/api/v1/strands/target/errors?limit=3&before=inc_last",
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("request");
            let mut bytes = [0; 4096];
            let count = stream.read(&mut bytes).await.expect("read");
            let request = String::from_utf8_lossy(&bytes[..count]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n[]")
                .await
                .expect("respond");
            request
        });
        let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
            .args(["--base-url", &base])
            .args(args)
            .env_remove("SANTI_SOUL_ID")
            .env_remove("SANTI_STRAND_ID")
            .output()
            .await
            .expect("client");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            server
                .await
                .expect("server")
                .starts_with(&format!("GET {expected} HTTP/1.1\r\n"))
        );
    }
}
