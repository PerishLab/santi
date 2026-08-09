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
