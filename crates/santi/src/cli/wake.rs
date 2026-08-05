use clap::Subcommand;

#[derive(Subcommand)]
pub enum Wake {
    #[command(about = "Inspect the wake lease owned by --soul/SANTI_SOUL_ID")]
    Status,
    #[command(about = "Grant and start a bounded three-round wake invitation")]
    Enable,
    #[command(about = "Revoke autonomous wake permission immediately")]
    Disable,
}
