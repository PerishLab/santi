use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use super::{Identity, Request};
use crate::config::{self, Resume};

pub(super) struct Plan {
    pub(super) executable: PathBuf,
    pub(super) base: String,
    pub(super) soul: String,
    pub(super) strand: String,
    pub(super) bearer: Option<String>,
}

impl Plan {
    pub(super) fn new(request: &Request<'_>, identity: &Identity) -> Result<Self> {
        Ok(Self {
            executable: config::executable()?,
            base: request.base.to_string(),
            soul: identity.soul.clone(),
            strand: identity.strand.clone(),
            bearer: request.bearer.map(str::to_string),
        })
    }

    pub(super) fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command.arg("tui");
        config::resume(
            &mut command,
            Resume {
                base: &self.base,
                soul: &self.soul,
                strand: &self.strand,
                bearer: self.bearer.as_deref(),
            },
        );
        command
    }

    pub(super) fn prepare(&self) -> Result<Command> {
        #[cfg(unix)]
        return self.prepared(ratatui::try_restore);

        #[cfg(not(unix))]
        anyhow::bail!("/reload is currently supported only on Unix");
    }

    #[cfg(unix)]
    pub(super) fn prepared<F>(&self, restore: F) -> Result<Command>
    where
        F: FnOnce() -> std::io::Result<()>,
    {
        restore().context("restore terminal before reload")?;
        Ok(self.command())
    }

    #[cfg(unix)]
    pub(super) fn exec(&self, mut command: Command) -> anyhow::Error {
        use std::os::unix::process::CommandExt;

        let error = command.exec();
        anyhow::Error::new(error).context(format!("exec {}", self.executable.display()))
    }

    #[cfg(not(unix))]
    pub(super) fn exec(&self, command: Command) -> anyhow::Error {
        anyhow::anyhow!(
            "/reload is currently supported only on Unix; {:?} was not run",
            command.get_program()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    use super::Plan;
    use crate::config;

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
        if config::env("SANTI_RELOAD_EXEC_PROBE_OUTPUT").is_some() {
            let script = config::env("SANTI_RELOAD_EXEC_PROBE_SCRIPT").expect("probe script");
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

        let mut child = Command::new(config::executable().expect("test executable"))
            .arg("--exact")
            .arg("client::tui::reload::tests::replaces")
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
}
