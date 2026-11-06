#[derive(Debug, clap::Args)]
pub struct Reply {
    #[arg(
        long,
        help = "Exact accepted inbox receipt, not the platform message ID"
    )]
    pub inbox: String,
    #[arg(
        long,
        requires = "text",
        help = "Caller-stable operation; reuse only for identical intent"
    )]
    pub operation: Option<String>,
    #[arg(
        long,
        requires = "operation",
        help = "Final plain text; atomically claims the window before platform dispatch"
    )]
    pub text: Option<String>,
    #[arg(
        long,
        requires = "text",
        help = "Wait up to 30 seconds for this exact platform operation; unknown never permits replay"
    )]
    pub watch: bool,
}
