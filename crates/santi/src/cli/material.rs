use super::Rendering;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Materials {
    pub id: Option<String>,
    #[arg(
        long,
        value_enum,
        help = "Legacy render adapter: inspect guidance or assembled prompt without changing bindings"
    )]
    pub kind: Option<Rendering>,
    #[command(subcommand)]
    pub command: Option<Reading>,
}

#[derive(Subcommand)]
pub enum Reading {
    #[command(
        about = "Resolve and pin a revision for the next provider round; repeating load refreshes it"
    )]
    Load {
        reference: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        operation: Option<String>,
    },
    #[command(about = "Remove this scope binding; sources, history and learned memory remain")]
    Drop {
        reference: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        operation: Option<String>,
    },
    #[command(
        about = "Query bindings, available sources, revisions or recorded request content; no load occurs"
    )]
    Query {
        reference: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        effective: bool,
        #[arg(long)]
        operation: Option<String>,
        #[arg(
            long,
            help = "Context or thinking ID; use all to list recorded requests"
        )]
        context: Option<String>,
        #[arg(long)]
        available: bool,
        #[arg(long)]
        blob: Option<String>,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Soul {
    pub id: Option<String>,
    #[command(subcommand)]
    pub command: Option<Person>,
}

#[derive(Subcommand)]
pub enum Person {
    Material(Materials),
}
