use super::serve;

#[tokio::test]
async fn refusal() {
    use std::process::Stdio;
    let server = serve(
        "400 Bad Request",
        r#"{"message":"job cwd must be a workspace URI: use soul://, soul://<path>, strand://, or strand://<path> with no parent traversal; omit cwd to use the runtime execution directory (not the creating shell cwd)"}"#,
    )
    .await;
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"));
    command.args([
        "--base-url",
        &server.base,
        "job",
        "create",
        "test",
        "true",
        "--cwd",
        "/private/secret",
    ]);
    command
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.env("SANTI_JOB_CREATE_CAPABILITY", "test-capability");
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), command.output())
        .await
        .expect("bounded invocation")
        .expect("run job create");
    let request = tokio::time::timeout(std::time::Duration::from_secs(2), server.request)
        .await
        .expect("bounded request capture")
        .expect("request captured");
    assert!(request.contains("/api/v1/jobs"));
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("job cwd must be a workspace URI"));
    assert!(!text.contains("/private/secret"));

    for (status, body) in [
        (
            "400 Bad Request",
            r#"{"message":"secret-body","command":"secret-command"}"#.to_string(),
        ),
        ("400 Bad Request", "secret-malformed".to_string()),
        (
            "400 Bad Request",
            format!("{}secret-oversize", " ".repeat(4097)),
        ),
        (
            "500 Internal Server Error",
            r#"{"message":"secret-internal"}"#.to_string(),
        ),
    ] {
        let server = serve(status, &body).await;
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
                .args([
                    "--base-url",
                    &server.base,
                    "job",
                    "create",
                    "secret-description",
                    "secret-command",
                    "--cwd",
                    "/private/secret",
                ])
                .env("SANTI_JOB_CREATE_CAPABILITY", "test-capability")
                .output(),
        )
        .await
        .expect("bounded refusal")
        .expect("run refusal");
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(text.contains("job creation failed with status"));
        assert!(!text.contains("secret"));
        assert!(output.stdout.is_empty());
        assert!(text.len() < 512);
    }

    let help = tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
        .args(["job", "create", "--help"])
        .output()
        .await
        .expect("job help");
    let text = String::from_utf8_lossy(&help.stdout);
    for form in ["soul://", "soul://<path>", "strand://", "strand://<path>"] {
        assert!(text.contains(form));
    }
    assert!(text.contains("runtime execution directory"));
    assert!(text.contains("not the creating shell cwd"));
}
