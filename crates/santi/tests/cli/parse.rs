use clap::Parser;
use santi::cli::{
    Cli, Command, Compact, Effect, Environment, Format, Job, Outcome, Strand, Strategy, Stream,
    Target, Turn, Webhook,
};
#[test]
fn receipt() {
    let parsed = Cli::try_parse_from(["santi", "receipt", "inbox_123"]).unwrap();
    let Command::Receipt { inbox } = parsed.command else {
        panic!("expected receipt command");
    };
    assert_eq!(inbox, "inbox_123");
}
#[test]
fn turns() {
    assert!(Cli::try_parse_from(["santi", "turn", "active"]).is_ok());
    let parsed = Cli::try_parse_from(["santi", "turn", "show", "turn_123"]).unwrap();
    let Command::Turn(Turn::Show { id }) = parsed.command else {
        panic!("expected turn show command");
    };
    assert_eq!(id, "turn_123");
    let parsed = Cli::try_parse_from(["santi", "turn", "stop", "turn_123"]).unwrap();
    let Command::Turn(Turn::Stop { id }) = parsed.command else {
        panic!("expected turn stop command");
    };
    assert_eq!(id, "turn_123");
}
#[test]
fn environment() {
    let parsed = Cli::try_parse_from([
        "santi",
        "env",
        "set",
        "soul",
        "soul_default",
        "STIM_BASE_URL",
        "https://stim.example.com",
    ])
    .unwrap();
    let Command::Environment(Environment::Set {
        scope,
        owner,
        name,
        value,
    }) = parsed.command
    else {
        panic!("expected environment set command");
    };
    assert_eq!(scope, Target::Soul);
    assert_eq!(owner, "soul_default");
    assert_eq!(name, "STIM_BASE_URL");
    assert_eq!(value, "https://stim.example.com");
}
#[test]
fn jobs() {
    let parsed = Cli::try_parse_from([
        "santi",
        "job",
        "create",
        "compile release",
        "cargo build --release",
        "--cwd",
        "strand://repo",
        "--timeout-seconds",
        "90",
        "--output-limit-bytes",
        "4096",
        "--remind-every-seconds",
        "30",
    ])
    .unwrap();
    let Command::Job(Job::Create {
        description,
        command,
        cwd,
        timeout,
        limit,
        remind,
        notify,
    }) = parsed.command
    else {
        panic!("expected job create");
    };
    assert_eq!(description, "compile release");
    assert_eq!(command, "cargo build --release");
    assert_eq!(cwd.as_deref(), Some("strand://repo"));
    assert_eq!(timeout, Some(90));
    assert_eq!(limit, Some(4096));
    assert_eq!(remind, Some(30));
    assert!(!notify);
    let parsed = Cli::try_parse_from([
        "santi", "job", "logs", "job_1", "--stream", "stderr", "--cursor", "12", "--limit", "80",
    ])
    .unwrap();
    let Command::Job(Job::Logs {
        id,
        stream,
        cursor,
        limit,
    }) = parsed.command
    else {
        panic!("expected job logs");
    };
    assert_eq!(id, "job_1");
    assert!(matches!(stream, Stream::Stderr));
    assert_eq!(cursor, "12");
    assert_eq!(limit, 80);
}

#[test]
fn webhooks() {
    let parsed = Cli::try_parse_from(["santi", "webhook", "list"]).unwrap();
    assert!(matches!(parsed.command, Command::Webhook(Webhook::List)));

    let parsed = Cli::try_parse_from([
        "santi",
        "webhook",
        "ensure",
        "secretary",
        "--adaptor",
        "github",
        "--soul",
        "soul_default",
        "--credential",
        "SANTI_WEBHOOK_GITHUB_SECRET",
    ])
    .unwrap();
    let Command::Webhook(Webhook::Ensure {
        name,
        adaptor,
        soul,
        strategy,
        credential,
    }) = parsed.command
    else {
        panic!("expected webhook ensure command");
    };
    assert_eq!(name, "secretary");
    assert_eq!(adaptor, "github");
    assert_eq!(soul, "soul_default");
    assert_eq!(strategy, Strategy::Thread);
    assert_eq!(credential, "SANTI_WEBHOOK_GITHUB_SECRET");
}

#[test]
fn effects() {
    let parsed = Cli::try_parse_from(["santi", "effect", "query", "effect_123"]).unwrap();
    let Command::Effect(Effect::Query { effect }) = parsed.command else {
        panic!("expected effect query");
    };
    assert_eq!(effect, "effect_123");

    let parsed = Cli::try_parse_from([
        "santi",
        "effect",
        "resolve",
        "effect_123",
        "--outcome",
        "not-applied",
        "--evidence",
        "marker absent",
    ])
    .unwrap();
    let Command::Effect(Effect::Resolve {
        effect,
        outcome,
        evidence,
    }) = parsed.command
    else {
        panic!("expected effect resolution");
    };
    assert_eq!(effect, "effect_123");
    assert_eq!(outcome, Outcome::Unapplied);
    assert_eq!(evidence, "marker absent");
}

#[test]
fn watch() {
    let parsed = Cli::try_parse_from(["santi", "strand", "send", "--watch", "hello"]).unwrap();
    let Command::Strand(Strand::Send {
        args,
        watch,
        format,
    }) = parsed.command
    else {
        panic!("expected strand send command");
    };
    assert_eq!(args, vec!["hello"]);
    assert!(watch);
    assert_eq!(format, Format::Filtered);

    assert!(
        Cli::try_parse_from(["santi", "strand", "send", "--watch-format", "raw", "hello"]).is_err()
    );

    let parsed = Cli::try_parse_from([
        "santi",
        "strand",
        "send",
        "--watch",
        "--watch-format",
        "raw",
        "hello",
    ])
    .unwrap();
    let Command::Strand(Strand::Send { format, .. }) = parsed.command else {
        panic!("expected strand send command");
    };
    assert_eq!(format, Format::Raw);

    let parsed = Cli::try_parse_from(["santi", "strand", "events", "ss_1"]).unwrap();
    let Command::Strand(Strand::Events { id, format }) = parsed.command else {
        panic!("expected strand events command");
    };
    assert_eq!(id.as_deref(), Some("ss_1"));
    assert_eq!(format, Format::Raw);

    let parsed =
        Cli::try_parse_from(["santi", "strand", "events", "ss_1", "--format", "filtered"]).unwrap();
    let Command::Strand(Strand::Events { format, .. }) = parsed.command else {
        panic!("expected strand events command");
    };
    assert_eq!(format, Format::Filtered);
}

#[test]
fn capsule() {
    let parsed = Cli::try_parse_from([
        "santi",
        "compact",
        "capsule",
        "--from",
        "1",
        "--to",
        "9",
        "--summary-file",
        "summary.md",
        "--source",
        "operator",
        "--reason",
        "recover budget",
        "--risk",
        "summary may omit detail",
        "--dry",
    ])
    .unwrap();
    let Command::Compact(Compact::Capsule {
        from,
        to,
        file,
        source,
        reason,
        risk,
        dry,
        ..
    }) = parsed.command
    else {
        panic!("expected compact capsule command");
    };
    assert_eq!(from, Some(1));
    assert_eq!(to, Some(9));
    assert_eq!(file.as_deref(), Some("summary.md"));
    assert_eq!(source, "operator");
    assert_eq!(reason, "recover budget");
    assert_eq!(risk, "summary may omit detail");
    assert!(dry);
}

#[test]
fn absorbs() {
    let take = |list: &str| {
        let mut args = vec!["santi", "compact", "exec", "--summary", "k", "--absorb"];
        args.push(list);
        let parsed = Cli::try_parse_from(args).unwrap();
        let Command::Compact(Compact::Exec { absorb, .. }) = parsed.command else {
            panic!("expected compact exec");
        };
        absorb
    };
    assert_eq!(take("compact_a,compact_b"), ["compact_a", "compact_b"]);
    assert_eq!(take("compact_a, compact_b"), ["compact_a", "compact_b"]);
    assert_eq!(take("[compact_a, compact_b]"), ["compact_a", "compact_b"]);
    let mut args = vec!["santi", "compact", "exec", "--summary", "k", "--absorb"];
    args.extend(["compact_a,compact_b", "--absorb", "compact_c"]);
    let parsed = Cli::try_parse_from(args).unwrap();
    let Command::Compact(Compact::Exec { absorb, .. }) = parsed.command else {
        panic!("expected compact exec");
    };
    assert_eq!(absorb, ["compact_a", "compact_b", "compact_c"]);
}

#[test]
fn empties() {
    let mut args = vec!["santi", "compact", "exec", "--summary", "k", "--absorb"];
    args.push("compact_a,,compact_b");
    let Err(error) = Cli::try_parse_from(args) else {
        panic!("empty slot id was accepted");
    };
    assert!(error.to_string().contains("must not be empty"), "{error}");
}
