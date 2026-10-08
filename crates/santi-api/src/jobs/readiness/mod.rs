use std::collections::BTreeMap;

use serde::Serialize;

#[cfg(target_os = "linux")]
mod environment;
#[cfg(target_os = "linux")]
mod manager;

#[derive(Serialize)]
pub struct Report {
    platform: &'static str,
    scope: &'static str,
    shell: Option<Shell>,
    checks: Vec<Check>,
    pub ok: bool,
}

#[derive(Serialize)]
struct Shell {
    source: &'static str,
    reference: Option<String>,
    entries: usize,
    client: Option<std::path::PathBuf>,
}

#[derive(Serialize)]
struct Check {
    name: &'static str,
    state: &'static str,
    detail: &'static str,
    action: Option<&'static str>,
}

#[cfg(target_os = "linux")]
impl Check {
    fn ready(name: &'static str, detail: &'static str) -> Self {
        Self {
            name,
            state: "ready",
            detail,
            action: None,
        }
    }

    fn missing(name: &'static str, detail: &'static str, action: &'static str) -> Self {
        Self {
            name,
            state: "missing",
            detail,
            action: Some(action),
        }
    }
}

pub async fn inspect(declared: &BTreeMap<String, String>) -> Report {
    #[cfg(target_os = "linux")]
    {
        let (shell, mut checks) = environment::shell(declared);
        checks.extend(manager::inspect().await);
        let ok = checks.iter().all(|check| check.state == "ready");
        Report {
            platform: "linux",
            scope: "global_shell",
            shell: Some(shell),
            checks,
            ok,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = declared;
        Report {
            platform: std::env::consts::OS,
            scope: "global_shell",
            shell: None,
            checks: vec![Check {
                name: "platform",
                state: "unsupported",
                detail: "This check certifies Linux systemd detached-job prerequisites only.",
                action: None,
            }],
            ok: false,
        }
    }
}
