use super::Material;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Materials {
    pub id: Option<String>,
    #[arg(long, value_enum)]
    pub kind: Option<Material>,
    #[command(subcommand)]
    pub command: Option<Reading>,
}

#[derive(Subcommand)]
pub enum Reading {
    #[command(about = "Query material metadata or a named blob; this does not load instructions")]
    Query {
        reference: Option<String>,
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
