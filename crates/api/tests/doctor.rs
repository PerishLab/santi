use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_santi-api"));
    command
        .env_clear()
        .env("HOME", root)
        .env("SANTI_HOME", root)
        .current_dir(root);
    command
}

fn config(root: &Path, name: &str) -> std::path::PathBuf {
    let path = root.join(name);
    std::fs::write(
        &path,
        r#"
provider = "probe"
[providers.probe]
kind = "openai_responses"
api_key = "DOCTOR_SECRET"
model = "gpt-6.1-sol"
reasoning_effort = "medium"
bytes = 500000
"#,
    )
    .unwrap();
    path
}

fn decode(output: Output) -> Value {
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("DOCTOR_SECRET"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("DOCTOR_SECRET"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn precedence() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let home = config(root, "santi.toml");
    let environment = config(root, "environment.toml");
    let argument = config(root, "argument.toml");
    let report = decode(command(root).arg("doctor").output().unwrap());
    assert_eq!(report["config"]["path"], home.to_str().unwrap());
    assert_eq!(report["config"]["source"], "home");
    assert_eq!(report["config"]["present"], true);
    let report = decode(
        command(root)
            .env("SANTI_CONFIG", &environment)
            .arg("doctor")
            .output()
            .unwrap(),
    );
    assert_eq!(report["config"]["path"], environment.to_str().unwrap());
    assert_eq!(report["config"]["source"], "SANTI_CONFIG");
    let report = decode(
        command(root)
            .env("SANTI_CONFIG", &environment)
            .args(["--config", "argument.toml", "doctor"])
            .output()
            .unwrap(),
    );
    assert_eq!(report["config"]["path"], argument.to_str().unwrap());
    assert_eq!(report["config"]["source"], "argument");
    assert_eq!(report["target"], "local_process");
    assert_eq!(report["provider"]["model"], "gpt-6.1-sol");
    assert_eq!(report["provider"]["effort"], "medium");
}

#[test]
fn absent() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path()).arg("doctor").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("santi --base-url"));
    let report = decode(output);
    assert_eq!(report["config"]["present"], false);
    assert_eq!(report["state"], "unbootstrapped");
    assert!(report["actions"][0].as_str().unwrap().contains("bootstrap"));
    assert!(!temp.path().join("runtime").exists());
}

#[test]
fn credentials() {
    let temp = tempfile::tempdir().unwrap();
    let path = config(temp.path(), "santi.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("DOCTOR_SECRET", "env://PROBE_KEY")).unwrap();
    let report = decode(command(temp.path()).arg("doctor").output().unwrap());
    assert_eq!(report["provider"]["model"], "gpt-6.1-sol");
    assert_eq!(report["provider"]["effort"], "medium");
    assert_eq!(report["provider"]["ok"], false);
    assert!(
        report["provider"]["error"]
            .as_str()
            .unwrap()
            .contains("PROBE_KEY")
    );
    assert!(
        report["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| { action.as_str().unwrap().contains("service's environment") })
    );
}

#[tokio::test]
async fn lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    config(root, "santi.toml");
    assert!(
        command(root)
            .arg("bootstrap")
            .output()
            .unwrap()
            .status
            .success()
    );
    let output = command(root).arg("doctor").output().unwrap();
    assert!(!output.status.success());
    let report = decode(output);
    assert_eq!(report["state"], "genesis_missing");
    assert_eq!(report["estate_bound"], true);
    assert!(report["actions"][0].as_str().unwrap().contains("serve"));
    let store = santi_core::Store::open(root.join("runtime/db"))
        .await
        .unwrap();
    assert!(store.soul(santi_core::GENESIS).await.unwrap().is_none());
    store
        .seed(santi_core::GENESIS, &santi_core::now())
        .await
        .unwrap();
    drop(store);
    let output = command(root).arg("doctor").output().unwrap();
    assert!(output.status.success());
    let report = decode(output);
    assert_eq!(report["state"], "ready");
    assert_eq!(report["ok"], true);
    assert_eq!(report["actions"], serde_json::json!([]));
}

#[test]
fn malformed() {
    let temp = tempfile::tempdir().unwrap();
    config(temp.path(), "santi.toml");
    std::fs::create_dir(temp.path().join("runtime")).unwrap();
    let path = temp.path().join("runtime/db");
    std::fs::write(&path, b"not an estate").unwrap();
    let report = decode(command(temp.path()).arg("doctor").output().unwrap());
    assert_eq!(report["state"], "unavailable");
    assert!(report["actions"][0].as_str().unwrap().contains("custody"));
    assert_eq!(std::fs::read(path).unwrap(), b"not an estate");
}
