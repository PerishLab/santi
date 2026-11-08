use std::path::Path;
use std::process::{Command, Output};

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_santi"))
        .env_clear()
        .env("HOME", home)
        .env("SANTI_HOME", home.join("data"))
        .env("SANTI_BASE_URL", "http://127.0.0.1:0")
        .env("SANTI_AUTH_TOKEN_URL", "invalid")
        .current_dir(home)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn local() {
    let seat = tempfile::tempdir().unwrap();
    let data = seat.path().join("data");
    std::fs::create_dir(&data).unwrap();
    std::fs::write(data.join("client.toml"), "invalid client configuration").unwrap();
    let output = run(seat.path(), &["skill", "list"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("no managed skill"));
    assert!(!data.join("state/skills.json").exists());
}

#[test]
fn failure() {
    let seat = tempfile::tempdir().unwrap();
    let state = seat.path().join("data/state");
    std::fs::create_dir_all(&state).unwrap();
    let ledger = state.join("skills.json");
    std::fs::write(&ledger, "invalid ledger").unwrap();
    let output = run(seat.path(), &["skill", "list"]);
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("santi skill:"), "{stderr}");
    assert_eq!(std::fs::read_to_string(ledger).unwrap(), "invalid ledger");
}

#[test]
fn help() {
    let seat = tempfile::tempdir().unwrap();
    let output = run(seat.path(), &["skill", "--help"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for deed in ["install", "upgrade", "status", "stage", "list", "uninstall"] {
        assert!(stdout.contains(deed), "{stdout}");
    }
}

#[test]
fn focused() {
    let seat = tempfile::tempdir().unwrap();
    for args in [
        vec!["skill", "--help"],
        vec!["skill", "install", "--help"],
        vec!["skill", "status", "--help"],
    ] {
        let output = run(seat.path(), &args);
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        for option in [
            "--base-url",
            "--api-key",
            "--auth-password",
            "--strand",
            "--soul",
        ] {
            assert!(!stdout.contains(option), "{stdout}");
        }
        assert!(stdout.contains("--help"));
    }
    let output = run(seat.path(), &["strand", "send", "--help"]);
    assert!(String::from_utf8_lossy(&output.stdout).contains("--base-url"));
    let output = run(
        seat.path(),
        &[
            "skill",
            "list",
            "--base-url",
            "http://127.0.0.1:0",
            "--api-key",
            "secret",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("secret"));
}
