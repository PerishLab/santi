use santi_model::inference::*;
use std::collections::BTreeMap;

struct Map(BTreeMap<&'static str, Defaults>);

impl Map {
    fn two() -> Self {
        Self(BTreeMap::from([
            (
                "alpha",
                Defaults {
                    model: "alpha-default".into(),
                    reasoning_effort: Some("low".into()),
                },
            ),
            (
                "beta",
                Defaults {
                    model: "beta-default".into(),
                    reasoning_effort: Some("high".into()),
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
        provider: Some("alpha".into()),
        model: Some("low".into()),
        reasoning_effort: None,
    };
    let high = Declaration {
        provider: Some("beta".into()),
        model: None,
        reasoning_effort: Some("medium".into()),
    };
    let merged = high.overlay(&lower);
    assert_eq!(merged.provider.as_deref(), Some("beta"));
    assert_eq!(merged.model.as_deref(), Some("low"));
    assert_eq!(merged.reasoning_effort.as_deref(), Some("medium"));
}

#[test]
fn switch() {
    let service = Declaration {
        provider: Some("alpha".into()),
        model: None,
        reasoning_effort: None,
    };
    let soul = Declaration::default();
    let strand = Declaration {
        provider: Some("beta".into()),
        model: None,
        reasoning_effort: None,
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
    assert_eq!(effective.provider, "beta");
    assert_eq!(effective.model, "beta-default");
    assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(effective.source.provider, Origin::Strand);
    assert_eq!(effective.source.model, Origin::Default);
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Default));
}

#[test]
fn survives() {
    let service = Declaration {
        provider: Some("alpha".into()),
        model: Some("service-model".into()),
        reasoning_effort: None,
    };
    let soul = Declaration::default();
    let strand = Declaration {
        provider: Some("beta".into()),
        model: None,
        reasoning_effort: None,
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
    assert_eq!(effective.provider, "beta");
    assert_eq!(effective.model, "service-model");
    assert_eq!(effective.source.model, Origin::Service);
    assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
}

#[test]
fn wins() {
    let service = Declaration {
        provider: Some("alpha".into()),
        model: Some("service-model".into()),
        reasoning_effort: None,
    };
    let soul = Declaration {
        provider: None,
        model: None,
        reasoning_effort: Some("medium".into()),
    };
    let strand = Declaration {
        provider: None,
        model: Some("strand-model".into()),
        reasoning_effort: None,
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
    assert_eq!(effective.model, "strand-model");
    assert_eq!(effective.reasoning_effort.as_deref(), Some("medium"));
    assert_eq!(effective.source.model, Origin::Strand);
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Soul));
    assert_eq!(effective.source.provider, Origin::Service);
}

#[test]
fn patch() {
    let base = Declaration {
        provider: Some("alpha".into()),
        model: Some("m".into()),
        reasoning_effort: Some("low".into()),
    };
    let patch: Patch =
        serde_json::from_str(r#"{"model":null,"reasoning_effort":"high"}"#).expect("patch");
    assert!(!patch.touches("provider"));
    assert!(patch.touches("model"));
    let applied = patch.apply(&base);
    assert_eq!(applied.provider.as_deref(), Some("alpha"));
    assert_eq!(applied.model, None);
    assert_eq!(applied.reasoning_effort.as_deref(), Some("high"));
    let cleared: Patch = serde_json::from_str(r#"{"provider":null}"#).expect("null");
    assert_eq!(cleared.apply(&base).provider, None);
}

#[test]
fn refuse() {
    assert!(serde_json::from_str::<Declaration>(r#"{"model":"m","other":"x"}"#).is_err());
    assert!(
        Declaration {
            model: Some("  ".into()),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
