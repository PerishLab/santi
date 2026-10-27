use plumb::observation::{Declaration, Run};

pub(crate) fn start(command: &crate::cli::Command) -> Option<Run> {
    let home = crate::config::home()?;
    Run::start(
        Declaration {
            producer: "santi",
            file: &home.join("config.toml"),
            buffer: home.join("state/locus"),
        },
        name(command),
    )
}

fn name(command: &crate::cli::Command) -> &'static str {
    use crate::cli::Command;
    match command {
        Command::Health => "health",
        Command::Schema => "schema",
        Command::Soul(_) => "soul",
        Command::Tui { .. } => "tui",
        Command::Errors { .. } => "errors",
        Command::Receipt { .. } => "receipt",
        Command::Effect(_) => "effect",
        Command::Strand(_) => "strand",
        Command::Compact(_) => "compact",
        Command::Job(_) => "job",
        Command::Alarm(_) => "alarm",
        Command::Turn(_) => "turn",
        Command::Environment(_) => "env",
        Command::Webhook(_) => "webhook",
    }
}
