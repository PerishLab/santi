use super::*;

#[test]
fn connect() {
    let home = tempfile::tempdir().unwrap();
    let args = [
        "--base-url",
        "http://127.0.0.1:0",
        "--api-key",
        "private-token",
        "health",
    ];
    let baseline = command(home.path()).args(args).output().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let observed = command(home.path()).args(args).output().unwrap();
    assert_eq!(baseline.status.code(), observed.status.code());
    assert_eq!(baseline.stdout, observed.stdout);
    assert_eq!(diagnostics(&baseline.stderr), diagnostics(&observed.stderr));
    assert_eq!(Recorder(home.path()).faults(), ["transport.connect"]);
    assert_eq!(Recorder(home.path()).commands(), ["health"]);
}

#[test]
fn timed() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let _ = request(&mut stream);
        std::thread::sleep(Duration::from_secs(45));
    });
    let started = Instant::now();
    let output = invoke(
        home.path(),
        &endpoint,
        &["--api-key", "private-token", "health"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() >= Duration::from_secs(29));
    assert_eq!(Recorder(home.path()).faults(), ["transport.timeout"]);
    assert_eq!(Recorder(home.path()).commands(), ["health"]);
    drop(server);
}

#[test]
fn auth() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let request = request(&mut stream);
        reply(&mut stream, "500 Internal Server Error", "{}");
        request
    });
    let output = command(home.path())
        .args([
            "--auth-token-url",
            &endpoint,
            "--auth-client-id",
            "private-client",
            "--auth-username",
            "private-user",
            "--auth-password",
            "private-password",
            "health",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        server
            .join()
            .unwrap()
            .contains("grant_type=client_credentials")
    );
    assert_eq!(Recorder(home.path()).faults(), ["auth.exchange"]);
    assert_eq!(Recorder(home.path()).commands(), ["health"]);
    let text = serde_json::to_string(&Recorder(home.path()).records()).unwrap();
    for secret in [
        "private-client",
        "private-user",
        "private-password",
        endpoint.as_str(),
    ] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn incomplete() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let _ = request(&mut stream);
        reply(&mut stream, "200 OK", "{}");
    });
    let output = command(home.path())
        .args([
            "--auth-token-url",
            &endpoint,
            "--auth-client-id",
            "private-client",
            "--auth-username",
            "private-user",
            "--auth-password",
            "private-password",
            "health",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    server.join().unwrap();
    assert_eq!(Recorder(home.path()).faults(), ["auth.exchange"]);
}

#[test]
fn announced() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let _ = request(&mut stream);
        write!(
            stream,
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: 64\r\n\r\n"
        )
        .unwrap();
        stream.flush().unwrap();
        std::thread::sleep(Duration::from_secs(5));
    });
    let started = Instant::now();
    let output = invoke(
        home.path(),
        &endpoint,
        &[
            "--api-key",
            "private-token",
            "strand",
            "events",
            "ss_private",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(Recorder(home.path()).faults(), ["http.unexpected"]);
    assert_eq!(Recorder(home.path()).commands(), ["strand.events"]);
    drop(server);
}

#[test]
fn paths() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let _ = request(&mut stream);
        reply(&mut stream, "200 OK", RESPONSE);
    });
    let output = invoke(
        home.path(),
        &endpoint,
        &["--api-key", "private-token", "strand", "get", "ss_private"],
    );
    assert_eq!(output.status.code(), Some(0));
    server.join().unwrap();
    assert_eq!(Recorder(home.path()).commands(), ["strand.get"]);
    let text = serde_json::to_string(&Recorder(home.path()).records()).unwrap();
    for secret in ["private-token", "ss_private", endpoint.as_str()] {
        assert!(!text.contains(secret));
    }
}

#[test]
fn refused() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    let (listener, endpoint) = listener();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let _ = request(&mut stream);
        reply(
            &mut stream,
            "400 Bad Request",
            "{\"message\":\"private-refusal\"}",
        );
    });
    let output = invoke(
        home.path(),
        &endpoint,
        &[
            "--api-key",
            "private-token",
            "--soul",
            "soul_private",
            "job",
            "create",
            "private description",
            "private command",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    server.join().unwrap();
    assert_eq!(Recorder(home.path()).faults(), ["http.unexpected"]);
    assert_eq!(Recorder(home.path()).commands(), ["job.create"]);
    let text = serde_json::to_string(&Recorder(home.path()).records()).unwrap();
    assert!(!text.contains("private description"));
    assert!(!text.contains("private command"));
    assert!(!text.contains("private-refusal"));
    assert!(!text.contains("private-capability"));
}
