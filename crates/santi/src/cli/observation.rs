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
    use crate::cli::*;
    use plumb::skill::command::Deed;
    match command {
        Command::Skill(deed) => match deed {
            Deed::Install { .. } => "skill.install",
            Deed::Upgrade { .. } => "skill.upgrade",
            Deed::Status { .. } => "skill.status",
            Deed::Stage { .. } => "skill.stage",
            Deed::List => "skill.list",
            Deed::Uninstall => "skill.uninstall",
        },
        Command::Health => "health",
        Command::Schema => "schema",
        Command::Soul(soul) => match soul.command.as_ref() {
            None => "soul",
            Some(Person::Material(materials)) => match materials.command.as_ref() {
                None => "soul.material",
                Some(Reading::Load { .. }) => "soul.material.load",
                Some(Reading::Drop { .. }) => "soul.material.drop",
                Some(Reading::Query { .. }) => "soul.material.query",
            },
        },
        Command::Tui { .. } => "tui",
        Command::Errors { .. } => "errors",
        Command::Incident { .. } => "incident",
        Command::Receipt { .. } => "receipt",
        Command::Query { .. } => "query",
        Command::Reply(_) => "reply",
        Command::Effect(effect) => match effect {
            EffectCommand::Query { .. } => "effect.query",
            EffectCommand::Resolve { .. } => "effect.resolve",
        },
        Command::Strand(strand) => match strand {
            StrandCommand::Material(materials) => match materials.command.as_ref() {
                None => "strand.material",
                Some(Reading::Load { .. }) => "strand.material.load",
                Some(Reading::Drop { .. }) => "strand.material.drop",
                Some(Reading::Query { .. }) => "strand.material.query",
            },
            StrandCommand::Create(_) => "strand.create",
            StrandCommand::End { .. } => "strand.end",
            StrandCommand::List => "strand.list",
            StrandCommand::Get { .. } => "strand.get",
            StrandCommand::Messages { .. } => "strand.messages",
            StrandCommand::Runtime { .. } => "strand.runtime",
            StrandCommand::ExecutionTail { .. } => "strand.execution-tail",
            StrandCommand::Budget { .. } => "strand.budget",
            StrandCommand::Errors { .. } => "strand.errors",
            StrandCommand::Fork { .. } => "strand.fork",
            StrandCommand::Drive { .. } => "strand.drive",
            StrandCommand::Send { .. } => "strand.send",
            StrandCommand::Events { .. } => "strand.events",
        },
        Command::Compact(compact) => match compact {
            CompactCommand::Exec { .. } => "compact.exec",
            CompactCommand::Capsule { .. } => "compact.capsule",
            CompactCommand::Query { .. } => "compact.query",
        },
        Command::Job(job) => match job {
            Job::Create { .. } => "job.create",
            Job::List => "job.list",
            Job::Query { .. } => "job.query",
            Job::Get { .. } => "job.get",
            Job::Cancel { .. } => "job.cancel",
            Job::Logs { .. } => "job.logs",
            Job::Ack { .. } => "job.ack",
        },
        Command::Alarm(alarm) => match alarm {
            alarm::Alarm::Set { .. } => "alarm.set",
            alarm::Alarm::List => "alarm.list",
            alarm::Alarm::Get { .. } => "alarm.get",
            alarm::Alarm::End { .. } => "alarm.end",
            alarm::Alarm::Fire { .. } => "alarm.fire",
        },
        Command::Turn(turn) => match turn {
            Turn::Active => "turn.active",
            Turn::Show { .. } => "turn.show",
            Turn::Stop { .. } => "turn.stop",
        },
        Command::Environment(environment) => match environment {
            Environment::List { .. } => "env.list",
            Environment::Set { .. } => "env.set",
            Environment::End { .. } => "env.end",
        },
        Command::Webhook(webhook) => match webhook {
            Webhook::List => "webhook.list",
            Webhook::Ensure { .. } => "webhook.ensure",
        },
        Command::Inference(inference) => match inference {
            Inference::Soul(Scope::Get { .. }) => "inference.soul.get",
            Inference::Soul(Scope::Set(_)) => "inference.soul.set",
            Inference::Strand(Scope::Get { .. }) => "inference.strand.get",
            Inference::Strand(Scope::Set(_)) => "inference.strand.set",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::name;
    use crate::cli::Cli;
    use clap::Parser;

    fn check(argv: &[&str], expected: &str) {
        let mut full = vec!["santi"];
        full.extend_from_slice(argv);
        let cli = Cli::try_parse_from(full).expect("declared CLI path parses");
        assert_eq!(name(&cli.command), expected, "{argv:?}");
    }

    #[test]
    fn paths() {
        check(&["health"], "health");
        check(&["schema"], "schema");
        check(&["skill", "install"], "skill.install");
        check(&["skill", "upgrade"], "skill.upgrade");
        check(&["skill", "status"], "skill.status");
        check(
            &[
                "skill",
                "stage",
                "--channel",
                "value",
                "--version",
                "value",
                "--path",
                "value",
            ],
            "skill.stage",
        );
        check(&["skill", "list"], "skill.list");
        check(&["skill", "uninstall"], "skill.uninstall");
        check(&["soul"], "soul");
        check(&["soul", "material"], "soul.material");
        check(&["soul", "material", "load", "value"], "soul.material.load");
        check(&["soul", "material", "drop", "value"], "soul.material.drop");
        check(&["soul", "material", "query"], "soul.material.query");
        check(&["tui"], "tui");
        check(&["errors"], "errors");
        check(&["incident", "value"], "incident");
        check(&["receipt", "value"], "receipt");
        check(&["query", "value"], "query");
        check(&["reply", "--inbox", "value"], "reply");
        check(&["effect", "query", "value"], "effect.query");
        check(
            &[
                "effect",
                "resolve",
                "value",
                "--outcome",
                "applied",
                "--evidence",
                "value",
            ],
            "effect.resolve",
        );
        check(&["strand", "material"], "strand.material");
        check(
            &["strand", "material", "load", "value"],
            "strand.material.load",
        );
        check(
            &["strand", "material", "drop", "value"],
            "strand.material.drop",
        );
        check(&["strand", "material", "query"], "strand.material.query");
        check(&["strand", "create"], "strand.create");
        check(&["strand", "end", "value"], "strand.end");
        check(&["strand", "list"], "strand.list");
        check(&["strand", "get"], "strand.get");
        check(&["strand", "messages"], "strand.messages");
        check(&["strand", "runtime"], "strand.runtime");
        check(&["strand", "execution-tail"], "strand.execution-tail");
        check(&["strand", "budget"], "strand.budget");
        check(&["strand", "errors"], "strand.errors");
        check(&["strand", "fork"], "strand.fork");
        check(&["strand", "drive"], "strand.drive");
        check(&["strand", "send", "value"], "strand.send");
        check(&["strand", "events"], "strand.events");
        check(&["compact", "exec", "--summary", "value"], "compact.exec");
        check(
            &[
                "compact",
                "capsule",
                "--summary",
                "value",
                "--reason",
                "value",
                "--risk",
                "value",
            ],
            "compact.capsule",
        );
        check(&["compact", "query", "--compact", "value"], "compact.query");
        check(&["job", "create", "value", "value"], "job.create");
        check(&["job", "list"], "job.list");
        check(&["job", "query"], "job.query");
        check(&["job", "get", "value"], "job.get");
        check(&["job", "cancel", "value"], "job.cancel");
        check(&["job", "logs", "value"], "job.logs");
        check(&["job", "ack", "value"], "job.ack");
        check(
            &[
                "alarm", "set", "value", "--cron", "value", "--text", "value",
            ],
            "alarm.set",
        );
        check(&["alarm", "list"], "alarm.list");
        check(&["alarm", "get", "value"], "alarm.get");
        check(&["alarm", "end", "value"], "alarm.end");
        check(
            &["alarm", "fire", "value", "--generation", "1"],
            "alarm.fire",
        );
        check(&["turn", "active"], "turn.active");
        check(&["turn", "show", "value"], "turn.show");
        check(&["turn", "stop", "value"], "turn.stop");
        check(&["env", "list", "soul", "value"], "env.list");
        check(
            &["env", "set", "soul", "value", "value", "value"],
            "env.set",
        );
        check(&["env", "end", "soul", "value", "value"], "env.end");
        check(&["webhook", "list"], "webhook.list");
        check(
            &[
                "webhook",
                "ensure",
                "value",
                "--adaptor",
                "value",
                "--soul",
                "value",
                "--credential",
                "value",
            ],
            "webhook.ensure",
        );
        check(&["inference", "soul", "get"], "inference.soul.get");
        check(
            &["inference", "soul", "set", "--model", "value"],
            "inference.soul.set",
        );
        check(&["inference", "strand", "get"], "inference.strand.get");
        check(
            &["inference", "strand", "set", "--model", "value"],
            "inference.strand.set",
        );
    }

    #[test]
    fn arguments() {
        check(
            &[
                "--api-key",
                "private-token",
                "--strand",
                "ss_private",
                "strand",
                "send",
                "private text",
            ],
            "strand.send",
        );
    }
}
