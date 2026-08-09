use std::ffi::OsStr;
use std::process::Command;

use santi::config::{Resume, resume};

#[test]
fn identity() {
    let mut command = Command::new("/tmp/santi");
    command
        .arg("tui")
        .env("SANTI_API_KEY", "inherited-key")
        .env("SANTI_AUTH_TOKEN_URL", "https://auth.example/token")
        .env("SANTI_AUTH_CLIENT_ID", "inherited-client")
        .env("SANTI_AUTH_USERNAME", "inherited-user")
        .env("SANTI_AUTH_PASSWORD", "inherited-password");

    resume(
        &mut command,
        Resume {
            base: "http://runtime.example",
            soul: "soul_luna",
            strand: "ss_direct",
            bearer: Some("resolved bearer"),
        },
    );

    assert_eq!(command.get_args().collect::<Vec<_>>(), [OsStr::new("tui")]);
    let environment = command.get_envs().collect::<Vec<_>>();
    for (name, expected) in [
        ("SANTI_API_URL", "http://runtime.example"),
        ("SANTI_SOUL_ID", "soul_luna"),
        ("SANTI_STRAND_ID", "ss_direct"),
        ("SANTI_API_KEY", "resolved bearer"),
        ("SANTI_AUTH_TOKEN_URL", ""),
        ("SANTI_AUTH_CLIENT_ID", ""),
        ("SANTI_AUTH_USERNAME", ""),
        ("SANTI_AUTH_PASSWORD", ""),
    ] {
        assert!(
            environment.iter().any(|(key, value)| {
                key == &OsStr::new(name) && value == &Some(OsStr::new(expected))
            }),
            "{name} did not have the exact replacement value"
        );
    }
    assert!(
        !command
            .get_args()
            .any(|argument| argument == OsStr::new("resolved bearer")),
        "the resolved bearer must not enter argv"
    );
}

#[test]
fn unresolved() {
    let mut command = Command::new("/tmp/santi");
    command
        .arg("tui")
        .env("SANTI_API_KEY", "inherited-key")
        .env("SANTI_AUTH_TOKEN_URL", "https://auth.example/token")
        .env("SANTI_AUTH_CLIENT_ID", "inherited-client")
        .env("SANTI_AUTH_USERNAME", "inherited-user")
        .env("SANTI_AUTH_PASSWORD", "inherited-password");

    resume(
        &mut command,
        Resume {
            base: "http://runtime.example",
            soul: "soul_luna",
            strand: "ss_direct",
            bearer: None,
        },
    );

    let environment = command.get_envs().collect::<Vec<_>>();
    for name in [
        "SANTI_API_KEY",
        "SANTI_AUTH_TOKEN_URL",
        "SANTI_AUTH_CLIENT_ID",
        "SANTI_AUTH_USERNAME",
        "SANTI_AUTH_PASSWORD",
    ] {
        assert!(
            environment
                .iter()
                .any(|(key, value)| { key == &OsStr::new(name) && value == &Some(OsStr::new("")) }),
            "{name} must be blanked when no bearer was resolved"
        );
    }
}

pub struct Identity {
    pub soul: String,
    pub strand: String,
}

pub struct Request<'a> {
    pub base: &'a str,
    pub bearer: Option<&'a str>,
}

#[allow(dead_code)]
#[path = "../../src/client/tui/reload.rs"]
mod plan;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use plan::Plan;

#[test]
fn sealed() {
    let plan = Plan {
        executable: PathBuf::from("/does/not/matter"),
        base: "http://runtime.example".to_string(),
        soul: "soul_luna".to_string(),
        strand: "ss_direct".to_string(),
        bearer: Some("secret bearer".to_string()),
    };
    let error = plan
        .prepared(|| Err(std::io::Error::other("restore failed")))
        .expect_err("restore failure must refuse reload");
    assert!(format!("{error:#}").contains("restore terminal before reload"));
}

#[test]
fn refuses() {
    let directory = tempfile::tempdir().expect("probe directory");
    let missing = directory.path().join("missing");
    let plan = Plan {
        executable: missing.clone(),
        base: "http://runtime.example".to_string(),
        soul: "soul_luna".to_string(),
        strand: "ss_direct".to_string(),
        bearer: Some("secret bearer".to_string()),
    };
    let error = plan.exec(plan.command());
    let detail = format!("{error:#}");
    assert!(detail.contains("exec "), "{detail}");
    assert!(
        detail.contains(missing.to_string_lossy().as_ref()),
        "{detail}"
    );
    assert!(!detail.contains("secret bearer"), "{detail}");
}

#[test]
fn replaces() {
    if std::env::var_os("SANTI_RELOAD_EXEC_PROBE_OUTPUT").is_some() {
        let script = std::env::var_os("SANTI_RELOAD_EXEC_PROBE_SCRIPT").expect("probe script");
        let plan = Plan {
            executable: script.into(),
            base: "http://runtime.example".to_string(),
            soul: "soul_luna".to_string(),
            strand: "ss_direct".to_string(),
            bearer: Some("secret bearer".to_string()),
        };
        let command = plan.command();
        let error = plan.exec(command);
        panic!("exec probe unexpectedly returned: {error:#}");
    }

    let directory = tempfile::tempdir().expect("probe directory");
    let report = directory.path().join("report");
    let script = directory.path().join("probe.sh");
    fs::write(
        &script,
        "#!/bin/sh\nprintf 'pid=%s\\n' \"$$\" > \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\nprintf 'arg=%s\\n' \"$@\" >> \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\nprintf 'base=%s\\n' \"$SANTI_API_URL\" >> \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\nprintf 'soul=%s\\n' \"$SANTI_SOUL_ID\" >> \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\nprintf 'strand=%s\\n' \"$SANTI_STRAND_ID\" >> \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\nprintf 'bearer=%s\\n' \"$SANTI_API_KEY\" >> \"$SANTI_RELOAD_EXEC_PROBE_OUTPUT\"\n",
    )
    .expect("write probe");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).expect("chmod probe");

    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .arg("--exact")
        .arg("reload::replaces")
        .arg("--nocapture")
        .env("SANTI_RELOAD_EXEC_PROBE_OUTPUT", &report)
        .env("SANTI_RELOAD_EXEC_PROBE_SCRIPT", &script)
        .spawn()
        .expect("spawn exec probe");
    let pid = child.id();
    let status = child.wait().expect("wait for exec probe");
    assert!(status.success(), "exec probe failed: {status}");
    let report = fs::read_to_string(report).expect("read exec probe report");
    assert!(report.contains(&format!("pid={pid}")), "{report}");
    assert!(report.lines().any(|line| line == "arg=tui"), "{report}");
    assert!(
        !report
            .lines()
            .any(|line| line.starts_with("arg=") && line.contains("secret bearer")),
        "{report}"
    );
    assert!(report.contains("base=http://runtime.example"), "{report}");
    assert!(report.contains("soul=soul_luna"), "{report}");
    assert!(report.contains("strand=ss_direct"), "{report}");
    assert!(report.contains("bearer=secret bearer"), "{report}");
}
