use std::{path::Path, process::Stdio, time::Duration};

use tokio::{io::AsyncReadExt, process::Command, time::timeout};

use super::{Check, environment};

const DEADLINE: Duration = Duration::from_secs(3);
const LIMIT: u64 = 512;

pub(super) async fn inspect() -> Vec<Check> {
    let mut checks = environment::session();
    let path = std::env::var_os("PATH").filter(|path| !path.is_empty());
    let mut tools = Vec::new();
    for name in ["systemctl", "systemd-run", "loginctl"] {
        let tool = path
            .as_ref()
            .and_then(|path| environment::executable(name, path));
        checks.push(if tool.is_some() {
            Check::ready(name, "The systemd tool is executable through this API process PATH.")
        } else {
            Check::missing(name, "The systemd tool is unavailable through this API process PATH.", "Install the host's systemd tools and include their directory in the API service process PATH; environment.PATH configures model-facing shells separately.")
        });
        tools.push(tool);
    }
    let observed = match tools[0].as_deref() {
        Some(tool) => {
            query(
                tool,
                &[
                    "--user",
                    "show",
                    "--property=SystemState",
                    "--value",
                    "--no-pager",
                ],
            )
            .await
        }
        None => None,
    };
    checks.push(if matches!(observed.as_deref(), Some("running" | "degraded")) {
        Check::ready("manager", "A systemd user manager is reachable through this process environment and accepts read-only queries.")
    } else {
        Check::missing("manager", "The user manager query failed, timed out, exceeded its output bound or reported a non-running state.", "Check systemctl --user status in the runtime user's service environment and restore its user-manager/bus connection.")
    });
    let uid = unsafe { libc::geteuid() }.to_string();
    let lingering = match tools[2].as_deref() {
        Some(tool) => {
            query(
                tool,
                &[
                    "show-user",
                    &uid,
                    "--property=Linger",
                    "--value",
                    "--no-pager",
                ],
            )
            .await
        }
        None => None,
    };
    checks.push(if lingering.as_deref() == Some("yes") {
        Check::ready("linger", "Lingering keeps this user's manager available after the API login session closes.")
    } else {
        Check::missing("linger", "Persistent user-manager lingering is disabled or could not be confirmed.", "Ask the host administrator to enable lingering for the runtime user with loginctl enable-linger, then repeat the check as that user.")
    });
    checks
}

async fn query(tool: &Path, arguments: &[&str]) -> Option<String> {
    let mut child = Command::new(tool)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let result = timeout(DEADLINE, async {
        let mut bytes = Vec::new();
        stdout.take(LIMIT + 1).read_to_end(&mut bytes).await.ok()?;
        if bytes.len() as u64 > LIMIT {
            return None;
        }
        let status = child.wait().await.ok()?;
        status
            .success()
            .then(|| String::from_utf8(bytes).ok())
            .flatten()
    })
    .await
    .ok()
    .flatten();
    if result.is_none() {
        let _ = child.kill().await;
    }
    result.map(|text| text.trim().to_string())
}
