use clap::Subcommand;

#[derive(Subcommand)]
pub enum Alarm {
    #[command(
        about = "Create or update one named declaration; identical sets preserve its generation. Cron uses five numeric fields in the endpoint host timezone, without missed-period backfill. Inspect projection.applied separately from declaration persistence"
    )]
    Set {
        name: String,
        #[arg(long)]
        cron: String,
        #[arg(long)]
        text: String,
        #[arg(
            long,
            help = "Expected current generation when changing an existing declaration"
        )]
        generation: Option<i64>,
    },
    #[command(about = "List the soul's declarations and actual endpoint projection status")]
    List,
    #[command(about = "Read one declaration and its latest observed endpoint state")]
    Get { id: String },
    #[command(
        about = "Idempotently end a declaration and remove its platform projection; accepted inputs remain"
    )]
    End { id: String },
    #[command(hide = true)]
    Fire {
        id: String,
        #[arg(long)]
        generation: i64,
    },
}
