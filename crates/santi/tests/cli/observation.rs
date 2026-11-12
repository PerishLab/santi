use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const RESPONSE: &str = "{\"status\":\"private-response\"}";
const ENABLED: &str = "[locus]\nenabled = true\nendpoint = 'http://127.0.0.1:0'\n";

mod faults;

fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    (listener, endpoint)
}

fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok((stream, _)) = listener.accept() {
            return stream;
        }
        assert!(Instant::now() < until, "client request missing");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut request = Vec::new();
    while !request.windows(4).any(|held| held == b"\r\n\r\n") {
        let mut chunk = [0u8; 1024];
        let length = stream.read(&mut chunk).unwrap();
        assert!(length > 0 && request.len() < 8192, "bounded headers");
        request.extend_from_slice(&chunk[..length]);
    }
    let end = request
        .windows(4)
        .position(|held| held == b"\r\n\r\n")
        .unwrap()
        + 4;
    let body = String::from_utf8_lossy(&request[..end])
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(end + body <= 8192, "bounded request body");
    while request.len() < end + body {
        let mut chunk = [0u8; 1024];
        let length = stream.read(&mut chunk).unwrap();
        assert!(length > 0, "complete request body");
        request.extend_from_slice(&chunk[..length]);
    }
    String::from_utf8_lossy(&request).to_string()
}

fn reply(stream: &mut TcpStream, status: &str, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

fn diagnostics(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.starts_with("locus engine diagnostic"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_santi"));
    command
        .current_dir(home)
        .env("SANTI_HOME", home)
        .env("SANTI_JOB_CREATE_CAPABILITY", "private-capability")
        .env_remove("SANTI_BASE_URL")
        .env_remove("SANTI_SOUL_ID")
        .env_remove("SANTI_STRAND_ID")
        .env_remove("SANTI_API_KEY")
        .env_remove("SANTI_AUTH_TOKEN_URL")
        .env_remove("SANTI_AUTH_CLIENT_ID")
        .env_remove("SANTI_AUTH_USERNAME")
        .env_remove("SANTI_AUTH_PASSWORD")
        .env_remove("SANTI_TOKEN_CACHE");
    command
}

fn invoke(home: &Path, endpoint: &str, args: &[&str]) -> Output {
    command(home)
        .args(["--base-url", endpoint])
        .args(args)
        .output()
        .unwrap()
}

fn run(home: &Path, status: &str, body: &str) -> Output {
    let (listener, endpoint) = listener();
    let status = status.to_string();
    let body = body.to_string();
    let server = std::thread::spawn(move || {
        let mut stream = accept(&listener);
        let request = request(&mut stream);
        reply(&mut stream, &status, &body);
        request
    });
    let output = invoke(home, &endpoint, &["--api-key", "private-token", "health"]);
    assert!(server.join().unwrap().contains("Bearer private-token"));
    output
}

struct Recorder<'a>(&'a Path);

impl Recorder<'_> {
    fn configure(&self, text: &str) {
        std::fs::write(self.0.join("config.toml"), text).unwrap();
    }

    fn records(&self) -> Vec<Value> {
        let mut records = Vec::new();
        for entry in std::fs::read_dir(self.0.join("state/locus")).unwrap() {
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

    fn finishes(&self) -> Vec<Value> {
        self.records()
            .into_iter()
            .filter(|atom| atom["payload"]["event"] == "cli.finish")
            .collect()
    }

    fn faults(&self) -> Vec<String> {
        self.finishes()
            .iter()
            .filter_map(|atom| atom["context"]["santi.fault"].as_str())
            .map(str::to_string)
            .collect()
    }

    fn commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = self
            .records()
            .iter()
            .filter_map(|atom| atom["context"]["santi.command"].as_str())
            .map(str::to_string)
            .collect();
        commands.sort();
        commands.dedup();
        commands
    }
}

#[test]
fn disabled() {
    let home = tempfile::tempdir().unwrap();
    let absent = run(home.path(), "200 OK", RESPONSE);
    Recorder(home.path()).configure("[locus]\nenabled = false\n");
    let disabled = run(home.path(), "200 OK", RESPONSE);
    assert_eq!(absent.status.code(), disabled.status.code());
    assert_eq!(absent.stdout, disabled.stdout);
    assert_eq!(absent.stderr, disabled.stderr);
    assert!(!home.path().join("state/locus").exists());
}

#[test]
fn buffered() {
    let home = tempfile::tempdir().unwrap();
    Recorder(home.path()).configure(ENABLED);
    for (status, code) in [("200 OK", 0), ("403 Forbidden", 1)] {
        let result = run(home.path(), status, RESPONSE);
        assert_eq!(result.status.code(), Some(code));
    }
    let unavailable = command(home.path())
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
    let atoms = Recorder(home.path()).records();
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
    let baseline = run(home.path(), "200 OK", RESPONSE);
    for config in [
        ENABLED,
        "[locus]\nenabled = 'private-token'\n",
        "[locus]\nenabled = true\nendpoint = ''\n",
    ] {
        Recorder(home.path()).configure(config);
        let result = run(home.path(), "200 OK", RESPONSE);
        assert_eq!(result.status.code(), baseline.status.code());
        assert_eq!(result.stdout, baseline.stdout);
        assert!(!result.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("private-token"));
    }
    let blocked = tempfile::tempdir().unwrap();
    std::fs::write(blocked.path().join("state"), "occupied").unwrap();
    Recorder(blocked.path()).configure(ENABLED);
    let result = run(blocked.path(), "200 OK", RESPONSE);
    assert!(result.status.success());
    assert_eq!(result.stdout, baseline.stdout);
    assert!(!result.stderr.is_empty());
}

#[test]
fn refused() {
    let home = tempfile::tempdir().unwrap();
    let refusal = "{\"code\":\"request.unauthorized\",\"message\":\"private-refusal\"}";
    let baseline = run(home.path(), "401 Unauthorized", refusal);
    assert_eq!(baseline.status.code(), Some(1));
    Recorder(home.path()).configure(ENABLED);
    let known = run(home.path(), "401 Unauthorized", refusal);
    assert_eq!(known.status.code(), baseline.status.code());
    assert_eq!(known.stdout, baseline.stdout);
    let unknown = run(home.path(), "403 Forbidden", RESPONSE);
    assert_eq!(unknown.status.code(), Some(1));
    assert_eq!(run(home.path(), "200 OK", RESPONSE).status.code(), Some(0));
    let finishes = Recorder(home.path()).finishes();
    assert_eq!(finishes.len(), 3);
    assert_eq!(
        Recorder(home.path()).faults(),
        ["request.unauthorized", "http.unexpected"]
    );
    for atom in &finishes {
        if atom["payload"]["code"] == 0 {
            assert!(atom["context"].get("santi.fault").is_none());
        }
    }
    let text = serde_json::to_string(&finishes).unwrap();
    assert!(!text.contains("private-refusal"));
    assert!(!text.contains("private-response"));
}
