use super::serve;

#[tokio::test]
async fn ownership() {
    for (args, target) in [
        (vec!["list"], "/api/v1/alarms?soul=soul_jarvis"),
        (
            vec!["get", "alarm_one"],
            "/api/v1/alarms/alarm_one?soul=soul_jarvis",
        ),
        (
            vec!["end", "alarm_one"],
            "/api/v1/alarms/alarm_one?soul=soul_jarvis",
        ),
    ] {
        let server = serve("200 OK", r#"{"alarms":[],"projection":{}}"#).await;
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new(env!("CARGO_BIN_EXE_santi"))
                .args(["--base-url", &server.base, "--soul", "soul_jarvis", "alarm"])
                .args(&args)
                .env_remove("SANTI_SOUL_ID")
                .env_remove("SANTI_STRAND_ID")
                .output(),
        )
        .await
        .expect("bounded alarm request")
        .expect("run alarm client");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let request = tokio::time::timeout(std::time::Duration::from_secs(2), server.request)
            .await
            .expect("bounded capture")
            .expect("request captured");
        let method = if args[0] == "end" { "DELETE" } else { "GET" };
        assert!(
            request.starts_with(&format!("{method} {target} HTTP/1.1")),
            "{request}"
        );
    }
}
