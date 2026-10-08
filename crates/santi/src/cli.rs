use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

pub const BASE: &str = "http://127.0.0.1:43307";

#[derive(Parser)]
#[command(
    name = "santi",
    version = plumb::version!("SANTI"),
    about = "HTTP client for a running santi runtime"
)]
pub struct Cli {
    #[arg(
        help = "Base URL of a running santi server. Falls back to SANTI_API_URL, then .local/secrets/santi.toml, then the local default. Only used by HTTP client commands",
        long,
        global = true,
        env = "SANTI_API_URL"
    )]
    pub base_url: Option<String>,

    #[arg(
        help = "Static bearer token sent on client requests. Falls back to SANTI_API_KEY, then .local/secrets/santi.toml. Transitional: prefer edge auth to reach santi behind forward-auth",
        long,
        global = true,
        env = "SANTI_API_KEY"
    )]
    pub api_key: Option<String>,

    #[arg(
        help = "Edge auth via authentik client_credentials. Explicit flags or env override .local/secrets/santi.toml. A complete set is exchanged for a cached short-lived JWT",
        long,
        global = true,
        env = "SANTI_AUTH_TOKEN_URL"
    )]
    pub auth_token_url: Option<String>,
    #[arg(long, global = true, env = "SANTI_AUTH_CLIENT_ID")]
    pub auth_client_id: Option<String>,
    #[arg(long, global = true, env = "SANTI_AUTH_USERNAME")]
    pub auth_username: Option<String>,
    #[arg(long, global = true, env = "SANTI_AUTH_PASSWORD")]
    pub auth_password: Option<String>,

    #[arg(
        help = "Default strand id used when a strand subcommand omits an explicit id. Falls back to SANTI_STRAND_ID. Empty/absent → an id must be passed",
        long,
        global = true,
        env = "SANTI_STRAND_ID"
    )]
    pub strand: Option<String>,

    #[arg(
        help = "Default soul addressed by `strand create`, `strand send`, and soul-owned job or wake commands. Falls back to SANTI_SOUL_ID. Empty/absent → strand create/send use the runtime default; job and wake reads or controls require one",
        long,
        global = true,
        env = "SANTI_SOUL_ID"
    )]
    pub soul: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "GET /api/v1/health")]
    Health,
    #[command(
        about = "Enter an interactive terminal seat for one soul and strand",
        long_about = "Enter an interactive terminal seat for one soul and strand. Use /jobs to list current-strand Jobs and /job N [stdout|stderr] [cursor] to inspect state, origin and bounded logs. Without explicit identities, tui creates server-assigned soul and strand ids exactly once. A newly listed soul has already published the exact requested initial memory. Ambiguous creation responses report outcome=state_unknown with do-not-retry list/resume guidance; recover a listed soul without --memory-file, and preserve a known soul when strand creation is ambiguous."
    )]
    Tui {
        #[arg(
            long = "memory-file",
            value_name = "PATH",
            help = "Seed a newly awakened soul from this UTF-8 file; invalid when --soul already names an existing soul"
        )]
        memory: Option<String>,
    },
    #[command(about = "Query canonical incidents by error scope")]
    Errors {
        #[arg(long, default_value = "runtime")]
        scope_kind: String,
        #[arg(long, default_value = "default")]
        scope_id: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    #[command(about = "Query one durable accepted-message obligation by inbox receipt id")]
    Receipt { inbox: String },
    #[command(about = "Query or explicitly resolve one external-effect attempt")]
    #[command(subcommand)]
    Effect(EffectCommand),
    #[command(about = "Strand resources under /api/v1/strands")]
    #[command(subcommand)]
    Strand(StrandCommand),
    #[command(about = "Compact a strand's own timeline, or query a compact's detail")]
    #[command(subcommand)]
    Compact(CompactCommand),
    #[command(about = "Soul-owned detached jobs under /api/v1/jobs")]
    #[command(subcommand)]
    Job(Job),
    #[command(
        subcommand,
        about = "Inspect or control one soul's autonomous wake lease"
    )]
    Wake(wake::Wake),
    #[command(about = "Turn controls under /api/v1/turns")]
    #[command(subcommand)]
    Turn(Turn),
    #[command(
        name = "env",
        about = "Manage soul or strand shell environment declarations"
    )]
    #[command(subcommand)]
    Environment(Environment),
    #[command(about = "Inspect or idempotently ensure webhook subscriptions")]
    #[command(subcommand)]
    Webhook(Webhook),
}

#[derive(Subcommand)]
pub enum EffectCommand {
    #[command(about = "GET /api/v1/effects/{id}")]
    Query { effect: String },
    #[command(
        about = "Resolve an unknown effect from operator-supplied evidence. This never retries a command or changes its receipt/turn state"
    )]
    Resolve {
        effect: String,
        #[arg(long, value_enum)]
        outcome: EffectOutcomeArg,
        #[arg(long)]
        evidence: String,
    },
}

#[derive(Subcommand)]
pub enum Turn {
    #[command(about = "List bounded active sibling turns for --soul, excluding --strand")]
    Active,
    #[command(about = "Idempotently stop one exact running turn")]
    Stop { id: String },
}

#[derive(Subcommand)]
pub enum Environment {
    #[command(about = "List declarations for one soul or strand")]
    List {
        #[arg(value_enum)]
        scope: EnvironmentScope,
        owner: String,
    },
    #[command(about = "Create or replace one declaration")]
    Set {
        #[arg(value_enum)]
        scope: EnvironmentScope,
        owner: String,
        name: String,
        value: String,
    },
    #[command(about = "Idempotently end one declaration")]
    End {
        #[arg(value_enum)]
        scope: EnvironmentScope,
        owner: String,
        name: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum EnvironmentScope {
    Soul,
    Strand,
}

impl EnvironmentScope {
    pub fn path(self) -> &'static str {
        match self {
            Self::Soul => "souls",
            Self::Strand => "strands",
        }
    }
}

#[derive(Subcommand)]
pub enum Webhook {
    #[command(about = "GET /api/v1/webhooks")]
    List,
    #[command(about = "Create the named subscription, confirm an identical one, or fail on drift")]
    Ensure {
        name: String,
        #[arg(long)]
        adaptor: String,
        #[arg(long)]
        soul: String,
        #[arg(long, value_enum, default_value_t = Strategy::Thread)]
        strategy: Strategy,
        #[arg(
            long,
            help = "Name of the server environment variable holding the signing secret"
        )]
        credential: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum Strategy {
    #[value(name = "per-thread")]
    Thread,
    Single,
}

impl Strategy {
    pub fn encode(self) -> &'static str {
        match self {
            Self::Thread => "per_thread",
            Self::Single => "single",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum EffectOutcomeArg {
    Applied,
    NotApplied,
}

impl EffectOutcomeArg {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::NotApplied => "not_applied",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum WatchFormat {
    #[value(
        help = "Human-readable milestones that omit stream chunks; send --watch also omits repeated turn states"
    )]
    Filtered,
    #[value(
        help = "Raw debugging output: SSE bytes for `strand events`, JSON event data for `send --watch`"
    )]
    Raw,
}

pub fn split_send_args(
    mut args: Vec<String>,
    defaults: &ClientDefaults,
) -> Result<(String, String)> {
    match args.len() {
        2 => {
            let text = args.pop().expect("len == 2");
            let id = args.pop().expect("len == 2");
            Ok((defaults.resolve_strand(Some(id))?, text))
        }
        1 => {
            let text = args.pop().expect("len == 1");
            Ok((defaults.resolve_strand(None)?, text))
        }
        _ => anyhow::bail!("send takes `<id> <text>` or `<text>`"),
    }
}

mod compact;
mod defaults;
mod job;
mod strand;
pub mod wake;
pub use compact::*;
pub use defaults::*;
pub use job::*;
pub use strand::*;
