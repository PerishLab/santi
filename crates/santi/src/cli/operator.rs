use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

const BLOCKED: [&str; 5] = [
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
];

#[derive(Subcommand)]
pub enum Operator {
    Audit(Forward),
    Deploy(Forward),
    Dev(Forward),
    Rollback(Forward),
    Scp(Forward),
    Ssh(Forward),
}

#[derive(Args)]
pub struct Forward {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

pub fn run(command: Operator) -> Result<()> {
    let (name, forward) = match command {
        Operator::Audit(args) => ("audit", args),
        Operator::Deploy(args) => ("deploy", args),
        Operator::Dev(args) => ("dev", args),
        Operator::Rollback(args) => ("rollback", args),
        Operator::Scp(args) => ("scp", args),
        Operator::Ssh(args) => ("ssh", args),
    };
    execute(name, forward)
}

fn execute(name: &str, forward: Forward) -> Result<()> {
    let root = root()?;
    let base = root.join("crates/santi/operator");
    let mut process = Command::new("deno");
    for key in BLOCKED {
        process.env_remove(key);
    }
    let status = process
        .args([
            OsString::from("run"),
            OsString::from("--no-prompt"),
            OsString::from("--allow-run"),
            OsString::from("--allow-read"),
            OsString::from("--allow-write"),
            OsString::from("--allow-env"),
            OsString::from("--allow-net"),
            OsString::from("--config"),
            base.join("deno.json").into_os_string(),
            OsString::from("--lock"),
            base.join("deno.lock").into_os_string(),
            OsString::from("--frozen=true"),
            base.join(format!("{name}.ts")).into_os_string(),
        ])
        .args(forward.args)
        .current_dir(&root)
        .status()
        .with_context(|| format!("start santi operator {name}"))?;
    if !status.success() {
        bail!("santi operator {name}: exited with {status}");
    }
    Ok(())
}

fn root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("find repository root")?;
    if !output.status.success() {
        bail!("git rev-parse: exited with {}", output.status);
    }
    let value = String::from_utf8(output.stdout).context("repository root is not UTF-8")?;
    Ok(PathBuf::from(value.trim_end()))
}
