#![cfg(target_os = "linux")]

use std::{
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::Value;

struct Fixture {
    root: tempfile::TempDir,
    tools: PathBuf,
    shell: PathBuf,
    session: PathBuf,
    _bus: UnixListener,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let tools = root.path().join("tools");
        let shell = root.path().join("shell");
        let session = root.path().join("session");
        for path in [&tools, &shell, &session] {
            std::fs::create_dir(path).unwrap();
        }
        let bus = UnixListener::bind(session.join("bus")).unwrap();
        let fixture = Self {
            root,
            tools,
            shell,
            session,
            _bus: bus,
        };
        fixture.tool("systemctl", "[ \"$*\" = '--user show --property=SystemState --value --no-pager' ] || exit 3\nprintf running");
        fixture.tool(
            "loginctl",
            "[ \"$1\" = show-user ] && [ \"$3\" = --property=Linger ] || exit 3\nprintf yes",
        );
        fixture.tool("systemd-run", "printf invoked > unexpected-job\nexit 3");
        let client = fixture.shell.join("santi");
        std::fs::write(
            &client,
            "#!/bin/sh\nprintf invoked > unexpected-client\nexit 3\n",
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o755)).unwrap();
        fixture.config(&fixture.shell.display().to_string());
        let output = fixture.command().arg("bootstrap").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let store = santi_core::Store::open(fixture.root.path().join("runtime/db"))
            .await
            .unwrap();
        store
            .seed(santi_core::GENESIS, &santi_core::now())
            .await
            .unwrap();
        drop(store);
        fixture
    }

    fn config(&self, path: &str) {
        let text = format!(
            "provider='probe'\n[environment]\nPATH={path:?}\n[providers.probe]\nkind='openai_responses'\napi_key='PROBE_SECRET'\nmodel='gpt-6.1-sol'\nreasoning_effort='medium'\nbytes=500000\n"
        );
        std::fs::write(self.root.path().join("santi.toml"), text).unwrap();
    }

    fn tool(&self, name: &str, body: &str) {
        let path = self.tools.join(name);
        let log = self.root.path().join("queries");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' '{name}' \"$*\" >> '{log}'\n{body}\n",
                log = log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_santi-api"));
        command
            .env_clear()
            .env("HOME", self.root.path())
            .env("SANTI_HOME", self.root.path())
            .env("PATH", &self.tools)
            .env("XDG_RUNTIME_DIR", &self.session)
            .current_dir(self.root.path());
        command
    }

    fn inspect(&self) -> (Output, Value) {
        let output = self.command().args(["doctor", "--jobs"]).output().unwrap();
        let report = decode(&output);
        (output, report)
    }
}

fn decode(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("PROBE_SECRET"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PROBE_SECRET"));
    serde_json::from_str(&stdout).unwrap()
}

fn state<'a>(report: &'a Value, name: &str) -> &'a str {
    report["jobs"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == name)
        .unwrap()["state"]
        .as_str()
        .unwrap()
}

#[tokio::test]
async fn ready() {
    let fixture = Fixture::new().await;
    let (output, report) = fixture.inspect();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(report["jobs"]["ok"], true);
    assert_eq!(report["jobs"]["platform"], "linux");
    assert_eq!(report["jobs"]["scope"], "global_shell");
    assert_eq!(
        report["jobs"]["shell"]["client"],
        fixture.shell.join("santi").to_str().unwrap()
    );
    assert_eq!(report["jobs"]["shell"]["entries"], 1);
    assert_eq!(report["jobs"]["shell"]["source"], "configuration");
    assert!(!fixture.root.path().join("unexpected-client").exists());
    assert!(!fixture.root.path().join("unexpected-job").exists());
    let queries = std::fs::read_to_string(fixture.root.path().join("queries")).unwrap();
    assert!(queries.contains("--user show --property=SystemState --value --no-pager"));
    assert!(queries.contains("--property=Linger --value --no-pager"));
    assert!(!queries.contains("enable-linger"));
    let output = fixture.command().arg("doctor").output().unwrap();
    assert!(output.status.success());
    assert!(decode(&output).get("jobs").is_none());
}

#[tokio::test]
async fn reference() {
    let fixture = Fixture::new().await;
    fixture.config("env://SHELL_PATH");
    let output = fixture
        .command()
        .env("SHELL_PATH", &fixture.shell)
        .args(["doctor", "--jobs"])
        .output()
        .unwrap();
    let report = decode(&output);
    assert!(output.status.success());
    assert_eq!(report["jobs"]["shell"]["reference"], "SHELL_PATH");
    let (output, report) = fixture.inspect();
    assert!(!output.status.success());
    assert_eq!(report["ok"], true);
    assert_eq!(state(&report, "path"), "missing");
    assert_eq!(state(&report, "client"), "missing");
    assert_eq!(state(&report, "systemctl"), "ready");
}

#[tokio::test]
async fn fallback() {
    let fixture = Fixture::new().await;
    let config = fixture.root.path().join("santi.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace(
            &format!("PATH={:?}\n", fixture.shell.display().to_string()),
            "",
        ),
    )
    .unwrap();
    let path = std::env::join_paths([&fixture.shell, &fixture.tools]).unwrap();
    let output = fixture
        .command()
        .env("PATH", path)
        .args(["doctor", "--jobs"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = decode(&output);
    assert_eq!(report["jobs"]["shell"]["source"], "process");
    assert_eq!(report["jobs"]["shell"]["entries"], 2);
}

#[tokio::test]
async fn client() {
    let fixture = Fixture::new().await;
    std::fs::set_permissions(
        fixture.shell.join("santi"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let (output, report) = fixture.inspect();
    assert!(!output.status.success());
    assert_eq!(state(&report, "client"), "missing");
    assert_eq!(state(&report, "path"), "ready");
    assert_eq!(state(&report, "manager"), "ready");
    fixture.config("");
    let (_, report) = fixture.inspect();
    assert_eq!(state(&report, "path"), "missing");
}

#[tokio::test]
async fn tools() {
    let fixture = Fixture::new().await;
    std::fs::remove_file(fixture.tools.join("systemd-run")).unwrap();
    let (_, report) = fixture.inspect();
    assert_eq!(state(&report, "systemd-run"), "missing");
    assert_eq!(state(&report, "systemctl"), "ready");
    std::fs::remove_file(fixture.tools.join("systemctl")).unwrap();
    let (_, report) = fixture.inspect();
    assert_eq!(state(&report, "systemctl"), "missing");
    assert_eq!(state(&report, "manager"), "missing");
    assert_eq!(state(&report, "loginctl"), "ready");
}

#[tokio::test]
async fn session() {
    let fixture = Fixture::new().await;
    let output = fixture
        .command()
        .env_remove("XDG_RUNTIME_DIR")
        .args(["doctor", "--jobs"])
        .output()
        .unwrap();
    let report = decode(&output);
    assert_eq!(state(&report, "runtime"), "missing");
    assert_eq!(state(&report, "bus"), "missing");
    assert_eq!(state(&report, "systemd-run"), "ready");
}

#[tokio::test]
async fn unavailable() {
    let fixture = Fixture::new().await;
    fixture.tool("systemctl", "printf PROBE_SECRET >&2\nexit 1");
    fixture.tool("loginctl", "printf no");
    let (output, report) = fixture.inspect();
    assert!(!output.status.success());
    assert_eq!(state(&report, "manager"), "missing");
    assert_eq!(state(&report, "linger"), "missing");
    assert_eq!(state(&report, "bus"), "ready");
}

#[tokio::test]
async fn bounded() {
    let fixture = Fixture::new().await;
    fixture.tool("systemctl", "exec /usr/bin/sleep 10");
    let start = std::time::Instant::now();
    let (_, report) = fixture.inspect();
    assert!(start.elapsed() < std::time::Duration::from_secs(6));
    assert_eq!(state(&report, "manager"), "missing");
    fixture.tool("systemctl", "/usr/bin/head -c 1024 /dev/zero");
    let (_, report) = fixture.inspect();
    assert_eq!(state(&report, "manager"), "missing");
}
