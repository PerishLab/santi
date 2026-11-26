use clap::{Args, Parser, Subcommand};

use crate::config::ConfigPartial;

#[derive(Args, Clone, Debug)]
pub struct Overrides {
    #[arg(long = "scenario")]
    pub scenario: Option<String>,
    #[arg(long = "reasoning-effort", value_name = "REASONING_EFFORT")]
    pub effort: Option<String>,
}

impl Overrides {
    pub fn partial(self) -> ConfigPartial {
        ConfigPartial {
            scenario: self.scenario.map(Some),
            effort: self.effort.map(Some),
            ..Default::default()
        }
    }
}

#[derive(Parser)]
#[command(
    name = "santi-api",
    version = plumb::version!("SANTI"),
    about = "santi runtime API server and local operator"
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<String>,
    #[arg(long, global = true, env = "SANTI_STRAND_ID")]
    pub strand: Option<String>,
    #[command(flatten)]
    pub over: Overrides,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Serve,
    #[command(about = "Initialize the local Keel estate and keep its sudo custody")]
    Bootstrap,
    #[command(name = "export-openapi")]
    Export,
    #[command(
        about = "Check the selected local config, estate and provider in this process environment; use santi health to query a running service"
    )]
    Doctor {
        #[arg(
            long,
            conflicts_with = "storage",
            help = "Inspect Linux detached-job prerequisites as the intended runtime user; checks global shell PATH and the persistent user manager without starting jobs"
        )]
        jobs: bool,
        #[arg(
            help = "Internal storage-only check used by deployment and recovery",
            long = "storage-only",
            hide = true
        )]
        storage: bool,
    },
    #[command(about = "Read durable tool activity from the local Keel estate")]
    Audit {
        #[arg(long)]
        turn: Option<String>,
        #[arg(long)]
        failed: bool,
        #[arg(short = 'n', long = "limit", default_value_t = 30)]
        limit: usize,
        #[arg(long, hide = true)]
        after: Option<String>,
    },
    #[command(about = "Operate directly on local runtime state")]
    #[command(subcommand)]
    Inbox(Inbox),
    #[command(about = "Inspect configured runtime capability authority")]
    #[command(subcommand)]
    Capability(Capability),
    #[command(name = "__job", hide = true)]
    #[command(subcommand)]
    Job(Job),
}

#[derive(Subcommand)]
pub enum Capability {
    #[command(about = "Print the active key id and Ed25519 public key")]
    Public,
}

#[derive(Subcommand)]
pub enum Job {
    Run,
    Finalize,
}

#[derive(Subcommand)]
pub enum Inbox {
    #[command(
        about = "Enqueue one `santi_system` record into a strand's durable inbox without a running server"
    )]
    Seed {
        #[arg(
            help = "The message text. For multi-line or shell-sensitive content, prefer --file or --stdin"
        )]
        #[arg(
            conflicts_with_all = ["file", "stdin"],
            required_unless_present_any = ["file", "stdin"]
        )]
        text: Option<String>,
        #[arg(
            help = "Read the message text from a file (or `-` for stdin)",
            long,
            value_name = "PATH",
            conflicts_with_all = ["text", "stdin"]
        )]
        file: Option<String>,
        #[arg(
            help = "Read the message text from stdin",
            long,
            conflicts_with_all = ["text", "file"]
        )]
        stdin: bool,
    },
}
