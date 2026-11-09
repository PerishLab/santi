use clap::{Subcommand, ValueEnum};

#[derive(Subcommand)]
pub enum Job {
    #[command(
        about = "Create a durable detached job from the current runtime shell invocation",
        long_about = "Create a durable detached job from the current runtime shell invocation. Each shell tool call can create one distinct Job. Identical request retries reuse that Job; use a new shell tool call for another Job."
    )]
    Create {
        description: String,
        command: String,
        #[arg(
            long,
            help = "Job working directory: soul://, soul://<path>, strand://, or strand://<path>; no parent traversal. Omit to use the runtime execution directory, not the creating shell cwd"
        )]
        cwd: Option<String>,
        #[arg(long)]
        timeout_seconds: Option<u64>,
        #[arg(long)]
        output_limit_bytes: Option<u64>,
        #[arg(long)]
        remind_every_seconds: Option<u64>,
        #[arg(
            long,
            help = "Request one terminal fact in the origin strand inbox after the job finishes (succeeded, failed, timed_out, cancelled or unknown); late work can depend on it"
        )]
        notify_on_terminal: bool,
    },
    #[command(about = "List jobs owned by --soul/SANTI_SOUL_ID")]
    List,
    #[command(
        about = "Query soul-visible jobs with bounded rows or an independent count",
        long_about = "Query soul-visible jobs with bounded rows or an independent count. Examples: santi job query --strand ss_x --state running --limit 20; santi job query --all-strands --count; santi query 'from Job where tag = \"job_...\" limit 5'. The public job id is the Job tag."
    )]
    Query {
        #[arg(
            long,
            help = "Query every strand visible to --soul/SANTI_SOUL_ID; overrides SANTI_STRAND_ID"
        )]
        all_strands: bool,
        #[arg(
            long,
            value_name = "STATE",
            action = clap::ArgAction::Append,
            help = "Repeatable job state filter"
        )]
        state: Vec<String>,
        #[arg(
            long,
            value_name = "N",
            help = "Maximum rows, 1..=200; default 20; mutually exclusive with --count"
        )]
        limit: Option<usize>,
        #[arg(
            long,
            help = "Return the independent total count instead of a bounded row page"
        )]
        count: bool,
    },
    #[command(about = "Get one soul-owned job")]
    Get { id: String },
    #[command(about = "Cancel one soul-owned job and its process tree")]
    Cancel { id: String },
    #[command(about = "Read one append-only job log stream from a byte cursor")]
    Logs {
        id: String,
        #[arg(long, value_enum, default_value_t = Stream::Stdout)]
        stream: Stream,
        #[arg(long, default_value = "0")]
        cursor: String,
        #[arg(long, default_value_t = 64 * 1024)]
        limit: usize,
    },
    #[command(about = "Acknowledge a terminal job and release retained sidecar state")]
    Ack { id: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}
