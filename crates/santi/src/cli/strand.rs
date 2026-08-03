use clap::Subcommand;

use super::*;

#[derive(Subcommand)]
pub enum StrandCommand {
    #[command(
        about = "Create a strand for --soul/SANTI_SOUL_ID, or for the runtime default",
        long_about = "Create exactly one strand. With --soul/SANTI_SOUL_ID, POST /api/v1/souls/{soul}/strands preserves that owner; without one, POST /api/v1/strands uses the runtime default. The client never retries. Only a contract-proven 4xx rejection reports outcome=not_created. Transport failure, an unreadable response body, 5xx or another non-client status, and an invalid success response report outcome=state_unknown with do-not-retry list/resume guidance; the client preserves a known parent soul and never invents the unknown strand id."
    )]
    Create,
    #[command(about = "GET /api/v1/strands")]
    List,
    #[command(about = "GET /api/v1/strands/{id} (id falls back to --strand/SANTI_STRAND_ID)")]
    Get { id: Option<String> },
    #[command(about = "GET /api/v1/strands/{id}/messages (id falls back to --strand)")]
    Messages { id: Option<String> },
    #[command(about = "GET /api/v1/strands/{id}/runtime (id falls back to --strand)")]
    Runtime { id: Option<String> },
    #[command(about = "GET /api/v1/strands/{id}/budget (id falls back to --strand)")]
    Budget { id: Option<String> },
    #[command(about = "GET /api/v1/strands/{id}/errors (id falls back to --strand)")]
    Errors {
        id: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    #[command(about = "POST /api/v1/strands/{id}/fork (id falls back to --strand)")]
    Fork { id: Option<String> },
    #[command(
        about = "POST /api/v1/strands/{id}/drive — explicitly redrive pending or failed receipts (id falls back to --strand)"
    )]
    Drive { id: Option<String> },
    #[command(
        about = "POST /api/v1/strands/{id}/send",
        long_about = "POST /api/v1/strands/{id}/send.\n\nPositional forms: `send <id> <text>` or `send <text>` (id then falls back to --strand/SANTI_STRAND_ID). Soul comes from --soul/SANTI_SOUL_ID.\n\nWith --watch, success proves only that this response's exact inbox receipt belongs to the requested strand and reached durable completed. Durable failed is an error. Pending or unavailable proof is never success: after sixty seconds without an event or ten minutes total, the command reports outcome=state_unknown, says not to resend, and provides receipt/status plus resume/redrive doors. Queued follow-on work may remain after success; --watch does not prove strand idleness."
    )]
    Send {
        #[arg(
            help = "Either `<id> <text>` or just `<text>`",
            num_args = 1..=2,
            required = true
        )]
        args: Vec<String>,
        #[arg(
            help = "Prove this accepted message's own exact durable receipt, then exit; this does not prove strand idle",
            long
        )]
        watch: bool,
        #[arg(
            help = "Output format for --watch. `raw` preserves the prior JSON-line debug stream"
        )]
        #[arg(
            long = "watch-format",
            value_enum,
            default_value_t = WatchFormat::Filtered,
            requires = "watch"
        )]
        watch_format: WatchFormat,
    },
    #[command(
        about = "GET /api/v1/strands/{id}/events — follows the SSE stream (id falls back to --strand). Runs until interrupted; `send --watch` instead stops after proving its accepted receipt"
    )]
    Events {
        id: Option<String>,
        #[arg(
            help = "Output format. Raw preserves the prior SSE byte stream",
            long,
            value_enum,
            default_value_t = WatchFormat::Raw
        )]
        format: WatchFormat,
    },
}
