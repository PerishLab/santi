use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};

const ENABLED: &str = "[locus]\nenabled = true\nendpoint = 'http://127.0.0.1:0'\n";

fn run(home: &Path, status: &str) -> Output {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let status = status.to_string();
    let server = std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut stream = loop {
            if let Ok((stream, _)) = listener.accept() {
                break stream;
            }
            assert!(std::time::Instant::now() < until, "client request missing");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.windows(4).any(|held| held == b"\r\n\r\n") {
            let mut chunk = [0u8; 1024];
            let length = stream.read(&mut chunk).unwrap();
            assert!(length > 0 && request.len() < 8192, "bounded headers");
            request.extend_from_slice(&chunk[..length]);
        }
        let request = String::from_utf8_lossy(&request).to_string();
        let body = "{\"status\":\"private-response\"}";
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        request
    });
    let output = Command::new(env!("CARGO_BIN_EXE_santi"))
        .current_dir(home)
        .env("SANTI_HOME", home)
        .env_remove("SANTI_API_KEY")
        .env_remove("SANTI_AUTH_TOKEN_URL")
        .env_remove("SANTI_AUTH_CLIENT_ID")
        .env_remove("SANTI_AUTH_USERNAME")
        .env_remove("SANTI_AUTH_PASSWORD")
        .args([
            "--base-url",
            &endpoint,
            "--api-key",
            "private-token",
            "health",
        ])
        .output()
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.contains("Bearer private-token"));
    output
}

fn configure(home: &Path, text: &str) {
    std::fs::write(home.join("config.toml"), text).unwrap();
}

fn records(home: &Path) -> Vec<Value> {
    let mut records = Vec::new();
    for entry in std::fs::read_dir(home.join("state/locus")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name == "active.jsonl" || name.starts_with("sealed-") {
            let content = std::fs::read_to_string(path).unwrap();
            records.extend(
                content
                    .lines()
                    .map(|line| serde_json::from_str::<Value>(line).unwrap()),
            );
        }
    }
    records
}

#[test]
fn disabled() {
    let home = tempfile::tempdir().unwrap();
    let absent = run(home.path(), "200 OK");
    configure(home.path(), "[locus]\nenabled = false\n");
    let disabled = run(home.path(), "200 OK");
    assert_eq!(absent.status.code(), disabled.status.code());
    assert_eq!(absent.stdout, disabled.stdout);
    assert_eq!(absent.stderr, disabled.stderr);
    assert!(!home.path().join("state/locus").exists());
}

#[test]
fn buffered() {
    let home = tempfile::tempdir().unwrap();
    configure(home.path(), ENABLED);
    for (status, code) in [("200 OK", 0), ("403 Forbidden", 1)] {
        let result = run(home.path(), status);
        assert_eq!(result.status.code(), Some(code));
    }
    let unavailable = Command::new(env!("CARGO_BIN_EXE_santi"))
        .current_dir(home.path())
        .env("SANTI_HOME", home.path())
        .env_remove("SANTI_AUTH_TOKEN_URL")
        .env_remove("SANTI_AUTH_CLIENT_ID")
        .env_remove("SANTI_AUTH_USERNAME")
        .env_remove("SANTI_AUTH_PASSWORD")
        .args([
            "--base-url",
            "http://127.0.0.1:0",
            "--api-key",
            "private-token",
            "health",
        ])
        .output()
        .unwrap();
    assert_eq!(unavailable.status.code(), Some(1));
    let atoms = records(home.path());
    assert!(atoms.iter().all(|atom| atom["producer"] == "santi"));
    assert!(
        atoms
            .iter()
            .all(|atom| atom["context"]["santi.command"] == "health")
    );
    for code in [0, 1] {
        assert!(atoms.iter().any(
            |atom| atom["payload"]["event"] == "cli.finish" && atom["payload"]["code"] == code
        ));
    }
    for edge in ["enter", "return"] {
        assert_eq!(
            atoms
                .iter()
                .filter(|atom| atom["source"]["edge"] == edge)
                .count(),
            9
        );
    }
    let text = serde_json::to_string(&atoms).unwrap();
    assert!(!text.contains("private-token"));
    assert!(!text.contains("private-response"));
    assert!(!text.contains("http://"));
    assert!(!text.contains("Bearer"));
}

#[test]
fn isolated() {
    let home = tempfile::tempdir().unwrap();
    let baseline = run(home.path(), "200 OK");
    for config in [
        ENABLED,
        "[locus]\nenabled = 'private-token'\n",
        "[locus]\nenabled = true\nendpoint = ''\n",
    ] {
        configure(home.path(), config);
        let result = run(home.path(), "200 OK");
        assert_eq!(result.status.code(), baseline.status.code());
        assert_eq!(result.stdout, baseline.stdout);
        assert!(!result.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("private-token"));
    }
    let blocked = tempfile::tempdir().unwrap();
    std::fs::write(blocked.path().join("state"), "occupied").unwrap();
    configure(blocked.path(), ENABLED);
    let result = run(blocked.path(), "200 OK");
    assert!(result.status.success());
    assert_eq!(result.stdout, baseline.stdout);
    assert!(!result.stderr.is_empty());
}
