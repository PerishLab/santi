use santi_model::inference::*;
use std::collections::BTreeMap;

struct Map(BTreeMap<&'static str, Defaults>);

impl Map {
    fn two() -> Self {
        Self(BTreeMap::from([
            (
                "gateway/alpha/work",
                Defaults {
                    model: "alpha-default".into(),
                    effort: Some("low".into()),
                },
            ),
            (
                "gateway/beta/work",
                Defaults {
                    model: "beta-default".into(),
                    effort: Some("high".into()),
                },
            ),
        ]))
    }
}

impl Directory for Map {
    fn defaults(&self, provider: &str) -> Result<Defaults, String> {
        self.0
            .get(provider)
            .cloned()
            .ok_or_else(|| format!("unknown provider {provider}"))
    }
}

#[test]
fn cascade() {
    let lower = Declaration {
        scenario: Some("gateway/alpha/work".into()),
        effort: None,
    };
    let high = Declaration {
        scenario: Some("gateway/beta/work".into()),
        effort: Some("medium".into()),
    };
    let merged = high.overlay(&lower);
    assert_eq!(merged.scenario.as_deref(), Some("gateway/beta/work"));
    assert_eq!(merged.effort.as_deref(), Some("medium"));
}

#[test]
fn switch() {
    let service = Declaration {
        scenario: Some("gateway/alpha/work".into()),
        effort: None,
    };
    let soul = Declaration::default();
    let strand = Declaration {
        scenario: Some("gateway/beta/work".into()),
        effort: None,
    };
    let effective = Effective::resolve(
        &[
            (&service, Origin::Service),
            (&soul, Origin::Soul),
            (&strand, Origin::Strand),
        ],
        &Map::two(),
    )
    .unwrap();
    assert_eq!(
        effective.selection,
        Selection::Scenario("gateway/beta/work".into())
    );
    assert_eq!(effective.model, "beta-default");
    assert_eq!(effective.effort.as_deref(), Some("high"));
    assert_eq!(
        effective.source.selection,
        Selection::Scenario(Origin::Strand)
    );
    assert_eq!(effective.source.model, Origin::Default);
    assert_eq!(effective.source.effort, Some(Origin::Default));
}

#[test]
fn wins() {
    let service = Declaration {
        scenario: Some("gateway/alpha/work".into()),
        effort: None,
    };
    let soul = Declaration {
        scenario: None,
        effort: Some("medium".into()),
    };
    let strand = Declaration {
        scenario: None,
        effort: None,
    };
    let effective = Effective::resolve(
        &[
            (&service, Origin::Service),
            (&soul, Origin::Soul),
            (&strand, Origin::Strand),
        ],
        &Map::two(),
    )
    .unwrap();
    assert_eq!(effective.model, "alpha-default");
    assert_eq!(effective.effort.as_deref(), Some("medium"));
    assert_eq!(effective.source.model, Origin::Default);
    assert_eq!(effective.source.effort, Some(Origin::Soul));
    assert_eq!(
        effective.source.selection,
        Selection::Scenario(Origin::Service)
    );
}

#[test]
fn patch() {
    let base = Declaration {
        scenario: Some("gateway/alpha/work".into()),
        effort: Some("low".into()),
    };
    let patch: Patch = serde_json::from_str(r#"{"reasoning_effort":"high"}"#).expect("patch");
    assert!(!patch.touches("scenario"));
    assert!(patch.touches("reasoning_effort"));
    let applied = patch.apply(&base);
    assert_eq!(applied.scenario, base.scenario);
    assert_eq!(applied.effort.as_deref(), Some("high"));
    let cleared: Patch = serde_json::from_str(r#"{"scenario":null}"#).expect("null");
    assert_eq!(cleared.apply(&base).scenario, None);
}

#[test]
fn grammar() {
    for value in [
        "gateway/family/review",
        "gateway/family/review.code",
        "gateway/family/review.code.rust",
    ] {
        assert!(santi::inference::path(value), "{value}");
    }
    for value in [
        "chat",
        "model-name",
        "/family/scene",
        "ga.te/family/scene",
        "g/f.x/scene",
        "g/f/",
        "g/f/a..b",
        "g/f/a.b.c.d",
        "g/f/aaa_",
        "g/f/two-words",
        "g/f/A",
        "g/f/abc1",
        "g/f/é",
        "g/f/a/",
        "g/f/a.",
    ] {
        let declaration = Declaration {
            scenario: Some(value.into()),
            ..Default::default()
        };
        assert!(declaration.validate().is_err(), "{value}");
    }
    for value in [
        r#"{"model":"m"}"#,
        r#"{"provider":"p"}"#,
        r#"{"scenario":"g/f/a","model":"wire"}"#,
    ] {
        assert!(
            serde_json::from_str::<Declaration>(value).is_err(),
            "{value}"
        );
        assert!(serde_json::from_str::<Patch>(value).is_err(), "{value}");
    }
}

#[test]
fn vocabulary() {
    use santi_model::catalog::Definition;
    for value in [
        "recall",
        "recall.correction",
        "recall.correction.later",
        "re-call",
        "Recall",
        "a.b.c.d",
    ] {
        let name = value.to_owned();
        let provider = Definition::Provider { name: name.clone() };
        let family = Definition::Family { name: name.clone() };
        let scenario = Definition::Scenario {
            name,
            criterion: "Revise a prior answer".into(),
        };
        assert_eq!(provider.validate().is_ok(), value == "recall");
        assert_eq!(family.validate().is_ok(), value == "recall");
        assert_eq!(scenario.validate().is_ok(), value.starts_with("recall"));
    }
    let offering = Definition::Offering {
        access: 1,
        family: 2,
        model: "vendor/model-4.1".into(),
    };
    assert!(offering.validate().is_ok());
}

#[test]
fn snapshot() {
    use serde_json::json;
    for line in [
        r#"{"provider":"old","scenario":"g/f/a","model":"m","source":{"scenario":"strand","model":"offering"}}"#,
        r#"{"scenario":"g/f/a","model":"m","source":{"provider":"strand","scenario":"strand","model":"offering"}}"#,
        r#"{"scenario":"g/f/a","model":"m","source":{"provider":"strand","model":"offering"}}"#,
        r#"{"scenario":"old","model":"m","source":{"scenario":"strand","model":"offering"}}"#,
    ] {
        assert!(serde_json::from_str::<Effective>(line).is_err(), "{line}");
    }
    for (key, selector) in [
        ("provider", "old"),
        ("provider", "gateway/family/non-canonical"),
        ("provider", "gateway/family/recall"),
        ("scenario", "gateway/family/recall"),
    ] {
        let wire = json!({
            key: selector,
            "model": "wire-model",
            "source": {key: "strand", "model": "offering"}
        });
        let value: Effective = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(value.selection.value(), selector);
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    }
}
