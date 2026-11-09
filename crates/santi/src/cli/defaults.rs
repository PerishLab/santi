use super::{Command, Inference, Scope, StrandCommand};
use crate::inference::Declaration;
use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, parser::ValueSource};

pub struct ClientDefaults {
    pub strand: Option<String>,
    pub soul: Option<String>,
    pub held: Declaration,
}

impl ClientDefaults {
    pub fn resolve_strand(&self, explicit: Option<String>) -> Result<String> {
        explicit
            .or_else(|| self.strand.clone())
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("no strand id: pass one or set --strand / SANTI_STRAND_ID")
            })
    }

    pub fn soul(&self) -> Option<&str> {
        self.soul
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    pub fn require(&self) -> Result<&str> {
        self.soul()
            .ok_or_else(|| anyhow::anyhow!("no soul id: pass --soul or set SANTI_SOUL_ID"))
    }
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

pub fn parse() -> std::result::Result<(super::Cli, bool), clap::Error> {
    let matches = super::Cli::command().get_matches();
    let explicit = matches.value_source("strand") == Some(ValueSource::CommandLine)
        || matches
            .subcommand()
            .is_some_and(|(_, sub)| sub.value_source("strand") == Some(ValueSource::CommandLine));
    let cli = super::Cli::from_arg_matches(&matches)?;
    Ok((cli, explicit))
}

pub fn declaration(command: &Command) -> Declaration {
    match command {
        Command::Inference(Inference::Soul(Scope::Set(edit)))
        | Command::Inference(Inference::Strand(Scope::Set(edit))) => edit.declaration(),
        Command::Strand(StrandCommand::Create(delta))
        | Command::Strand(StrandCommand::Fork { delta, .. }) => delta.declaration(),
        _ => Declaration::default(),
    }
}
