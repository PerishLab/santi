use clap::Parser;
use santi::cli::{Cli, Command, Delta, Edit, Field, Inference, Scope, StrandCommand};
use santi::inference::{Declaration, Patch};
use serde_json::json;

fn held() -> Declaration {
    Declaration {
        provider: Some("held-provider".into()),
        model: Some("held-model".into()),
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
        "--provider",
        "edge",
        "--model",
        "m1",
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
            provider: Some("edge".into()),
            model: Some("m1".into()),
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
        "provider,model",
        "--clear",
        "reasoning-effort",
        "ss_one",
    ])
    .unwrap();
    let Command::Inference(Inference::Strand(Scope::Set(edit))) = parsed.command else {
        panic!("expected strand inference set");
    };
    assert_eq!(edit.id.as_deref(), Some("ss_one"));
    assert_eq!(
        edit.clear,
        vec![Field::Provider, Field::Model, Field::Effort]
    );

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
        provider: None,
        model: None,
        reasoning_effort: None,
        clear: vec![Field::Model],
        ..Default::default()
    };
    assert_eq!(
        edit.patch(&held()).unwrap(),
        Patch {
            provider: Some(Some("held-provider".into())),
            model: Some(None),
            reasoning_effort: None,
        }
    );

    let edit = Edit {
        provider: Some("cli-provider".into()),
        model: None,
        reasoning_effort: None,
        clear: vec![Field::Provider],
        ..Default::default()
    };
    assert!(edit.patch(&held()).is_err());
}

#[test]
fn rejects() {
    assert!(Edit::default().patch(&Declaration::default()).is_err());

    let empty = Edit {
        provider: Some("  ".into()),
        ..Default::default()
    };
    assert!(empty.patch(&Declaration::default()).is_err());

    let unknown = Edit {
        clear: vec![Field::Model],
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
            provider: Some("p".into()),
            ..Default::default()
        }
        .body(&Declaration::default())
        .unwrap(),
        Some(json!({"inference": {"provider": "p"}}))
    );
    assert_eq!(
        Delta::default().body(&held()).unwrap(),
        Some(json!({"inference": {"provider": "held-provider", "model": "held-model"}}))
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
            provider: Some("p".into()),
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

    let parsed = Cli::try_parse_from(["santi", "strand", "create", "--model", "m"]).unwrap();
    let Command::Strand(StrandCommand::Create(delta)) = parsed.command else {
        panic!("expected strand create");
    };
    assert_eq!(
        delta.body(&Declaration::default()).unwrap(),
        Some(json!({"inference": {"model": "m"}}))
    );

    assert!(
        Cli::try_parse_from(["santi", "strand", "create", "--inherit", "--model", "m"]).is_err()
    );

    let parsed = Cli::try_parse_from(["santi", "strand", "fork", "ss_parent"]).unwrap();
    let Command::Strand(StrandCommand::Fork { id, delta }) = parsed.command else {
        panic!("expected strand fork");
    };
    assert_eq!(id.as_deref(), Some("ss_parent"));
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
    for flag in ["--model", "--provider", "--reasoning-effort"] {
        assert!(
            Cli::try_parse_from(["santi", "strand", "send", flag, "value", "hello"]).is_err(),
            "send must reject {flag}"
        );
    }
    assert!(Cli::try_parse_from(["santi", "strand", "send", "hello"]).is_ok());
}
