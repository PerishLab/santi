use clap::Parser;
use santi::cli::{Cli, Command, Delta, Edit, Field, Inference, Scope, StrandCommand};
use santi::inference::{Declaration, Patch};
use serde_json::json;

fn held() -> Declaration {
    Declaration {
        scenario: Some("held/family/work".into()),
        reasoning_effort: None,
    }
}

#[test]
fn reads() {
    let parsed = Cli::try_parse_from(["santi", "inference", "soul", "get"]).unwrap();
    let Command::Inference(Inference::Soul(Scope::Get { id })) = parsed.command else {
        panic!("expected soul inference get");
    };
    assert_eq!(id, None);

    let parsed = Cli::try_parse_from(["santi", "inference", "strand", "get", "ss_one"]).unwrap();
    let Command::Inference(Inference::Strand(Scope::Get { id })) = parsed.command else {
        panic!("expected strand inference get");
    };
    assert_eq!(id.as_deref(), Some("ss_one"));
}

#[test]
fn sets() {
    let parsed = Cli::try_parse_from([
        "santi",
        "inference",
        "soul",
        "set",
        "soul_one",
        "--scenario",
        "edge/family/work",
        "--reasoning-effort",
        "high",
    ])
    .unwrap();
    let Command::Inference(Inference::Soul(Scope::Set(edit))) = parsed.command else {
        panic!("expected soul inference set");
    };
    assert_eq!(edit.id.as_deref(), Some("soul_one"));
    assert_eq!(
        edit.declaration(),
        Declaration {
            scenario: Some("edge/family/work".into()),
            reasoning_effort: Some("high".into()),
        }
    );
}

#[test]
fn clears() {
    let parsed = Cli::try_parse_from([
        "santi",
        "inference",
        "strand",
        "set",
        "--clear",
        "scenario",
        "--clear",
        "reasoning-effort",
        "ss_one",
    ])
    .unwrap();
    let Command::Inference(Inference::Strand(Scope::Set(edit))) = parsed.command else {
        panic!("expected strand inference set");
    };
    assert_eq!(edit.id.as_deref(), Some("ss_one"));
    assert_eq!(edit.clear, vec![Field::Scenario, Field::Effort]);

    let parsed = Cli::try_parse_from([
        "santi",
        "inference",
        "soul",
        "set",
        "--clear",
        "reasoning_effort",
        "soul_one",
    ])
    .unwrap();
    let Command::Inference(Inference::Soul(Scope::Set(edit))) = parsed.command else {
        panic!("expected soul inference set");
    };
    assert_eq!(edit.clear, vec![Field::Effort]);
}

#[test]
fn patch() {
    let edit = Edit {
        scenario: None,
        reasoning_effort: None,
        clear: vec![Field::Scenario],
        ..Default::default()
    };
    assert_eq!(
        edit.patch(&held()).unwrap(),
        Patch {
            scenario: Some(None),
            reasoning_effort: None,
        }
    );

    let edit = Edit {
        scenario: Some("cli/family/work".into()),
        reasoning_effort: None,
        clear: vec![Field::Scenario],
        ..Default::default()
    };
    assert!(edit.patch(&held()).is_err());
}

#[test]
fn rejects() {
    for flag in ["--provider", "--model"] {
        for mut args in [
            vec!["santi", "inference", "soul", "set"],
            vec!["santi", "inference", "strand", "set"],
            vec!["santi", "strand", "create"],
            vec!["santi", "strand", "fork", "ss_parent"],
        ] {
            args.extend([flag, "edge"]);
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
    assert!(Edit::default().patch(&Declaration::default()).is_err());

    let empty = Edit {
        scenario: Some("  ".into()),
        ..Default::default()
    };
    assert!(empty.patch(&Declaration::default()).is_err());

    let unknown = Edit {
        clear: vec![Field::Scenario],
        ..Default::default()
    };
    assert!(unknown.patch(&Declaration::default()).is_ok());

    assert!(
        Cli::try_parse_from([
            "santi",
            "inference",
            "soul",
            "set",
            "soul_one",
            "--clear",
            "tempo"
        ])
        .is_err()
    );
    assert!(Cli::try_parse_from(["santi", "inference", "owner", "get"]).is_err());
    assert!(
        Cli::try_parse_from(["santi", "inference", "soul", "get", "--clear", "model"]).is_err()
    );
}

#[test]
fn bodies() {
    assert!(
        Delta::default()
            .body(&Declaration::default())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        Delta {
            scenario: Some("edge/family/work".into()),
            ..Default::default()
        }
        .body(&Declaration::default())
        .unwrap(),
        Some(json!({"inference": {"scenario": "edge/family/work"}}))
    );
    assert_eq!(
        Delta::default().body(&held()).unwrap(),
        Some(json!({"inference": {"scenario": "held/family/work"}}))
    );
    assert_eq!(
        Delta {
            inherit: true,
            ..Default::default()
        }
        .body(&Declaration::default())
        .unwrap(),
        Some(json!({"inference": {}}))
    );
    assert!(
        Delta {
            inherit: true,
            scenario: Some("edge/family/work".into()),
            ..Default::default()
        }
        .body(&Declaration::default())
        .is_err()
    );
}

#[test]
fn creates() {
    let parsed = Cli::try_parse_from(["santi", "strand", "create"]).unwrap();
    let Command::Strand(StrandCommand::Create(delta)) = parsed.command else {
        panic!("expected strand create");
    };
    assert!(delta.body(&Declaration::default()).unwrap().is_none());

    let parsed = Cli::try_parse_from([
        "santi",
        "strand",
        "create",
        "--scenario",
        "edge/family/work",
    ])
    .unwrap();
    let Command::Strand(StrandCommand::Create(delta)) = parsed.command else {
        panic!("expected strand create");
    };
    assert_eq!(
        delta.body(&Declaration::default()).unwrap(),
        Some(json!({"inference": {"scenario": "edge/family/work"}}))
    );

    assert!(
        Cli::try_parse_from([
            "santi",
            "strand",
            "create",
            "--inherit",
            "--scenario",
            "edge/family/work"
        ])
        .is_err()
    );

    let parsed = Cli::try_parse_from(["santi", "strand", "fork", "ss_parent"]).unwrap();
    let Command::Strand(StrandCommand::Fork { id, delta, at }) = parsed.command else {
        panic!("expected strand fork");
    };
    assert_eq!(id.as_deref(), Some("ss_parent"));
    assert_eq!(at, None);
    assert!(delta.body(&Declaration::default()).unwrap().is_none());

    let parsed =
        Cli::try_parse_from(["santi", "strand", "fork", "--inherit", "ss_parent"]).unwrap();
    let Command::Strand(StrandCommand::Fork { delta, .. }) = parsed.command else {
        panic!("expected strand fork");
    };
    assert_eq!(
        delta.body(&Declaration::default()).unwrap(),
        Some(json!({"inference": {}}))
    );
}

#[test]
fn send() {
    for flag in ["--scenario", "--model", "--provider", "--reasoning-effort"] {
        assert!(
            Cli::try_parse_from(["santi", "strand", "send", flag, "value", "hello"]).is_err(),
            "send must reject {flag}"
        );
    }
    assert!(Cli::try_parse_from(["santi", "strand", "send", "hello"]).is_ok());
}

#[test]
fn position() {
    let parsed = Cli::try_parse_from(["santi", "strand", "fork", "ss_parent", "--at", "0"])
        .expect("historical fork");
    let Command::Strand(StrandCommand::Fork { at, .. }) = parsed.command else {
        panic!("expected fork");
    };
    assert_eq!(at, Some(0));
    for invalid in ["-1", "9223372036854775808", "wrong"] {
        assert!(
            Cli::try_parse_from([
                "santi",
                "strand",
                "fork",
                "ss_parent",
                &format!("--at={invalid}")
            ])
            .is_err()
        );
    }
}
