use radicula_config::Cascade;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

mod grammar;
mod selection;
pub use grammar::{path, scenario, word};
pub use selection::Selection;

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq, Cascade)]
#[cascade(strict)]
#[serde(deny_unknown_fields)]
#[schema(as = inference::Declaration)]
pub struct Declaration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    #[cascade(name = "reasoning_effort")]
    #[serde(
        default,
        rename = "reasoning_effort",
        skip_serializing_if = "Option::is_none"
    )]
    pub effort: Option<String>,
}

impl Declaration {
    pub fn empty(&self) -> bool {
        self.scenario.is_none() && self.effort.is_none()
    }

    pub fn partial(&self) -> DeclarationPartial {
        DeclarationPartial {
            scenario: self.scenario.clone().map(Some),
            effort: self.effort.clone().map(Some),
        }
    }

    pub fn overlay(&self, lower: &Self) -> Self {
        lower.clone().merge(self.partial())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.scenario.as_deref().is_some_and(|value| !path(value)) {
            return Err("inference scenario requires provider/family/scene with one to three lowercase scene words joined by dots".into());
        }
        if self
            .effort
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("inference reasoning_effort must not be empty".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schema(as = inference::Patch)]
pub struct Patch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "nullable"
    )]
    pub scenario: Option<Option<String>>,
    #[serde(
        default,
        rename = "reasoning_effort",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "nullable"
    )]
    pub effort: Option<Option<String>>,
}

impl Patch {
    pub fn apply(&self, base: &Declaration) -> Declaration {
        base.clone().merge(DeclarationPartial {
            scenario: self.scenario.clone(),
            effort: self.effort.clone(),
        })
    }

    pub fn touches(&self, field: &str) -> bool {
        match field {
            "scenario" => self.scenario.is_some(),
            "reasoning_effort" => self.effort.is_some(),
            _ => false,
        }
    }
}

fn nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = inference::Origin)]
pub enum Origin {
    #[serde(rename = "provider_default")]
    Default,
    Offering,
    Service,
    Soul,
    Strand,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schema(as = inference::Source)]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Box<Route>>,
    #[serde(flatten)]
    #[schema(inline)]
    pub selection: Selection<Origin>,
    pub model: Origin,
    #[serde(rename = "reasoning_effort", skip_serializing_if = "Option::is_none")]
    pub effort: Option<Origin>,
}

mod route;
pub use route::{Label, Route};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Defaults {
    pub model: String,
    pub effort: Option<String>,
}

pub trait Directory {
    fn defaults(&self, scenario: &str) -> Result<Defaults, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(remote = "Self", deny_unknown_fields)]
#[schema(as = inference::Effective)]
pub struct Effective {
    #[serde(flatten)]
    #[schema(inline)]
    pub selection: Selection<String>,
    pub model: String,
    #[serde(rename = "reasoning_effort", skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    pub source: Source,
}

impl Effective {
    pub fn resolve(
        layers: &[(&Declaration, Origin)],
        providers: &dyn Directory,
    ) -> Result<Self, String> {
        for (declaration, _) in layers {
            declaration.validate()?;
        }
        let mut merged = Declaration::default();
        for (declaration, _) in layers {
            merged = merged.merge(declaration.partial());
        }
        let scenario = merged
            .scenario
            .ok_or_else(|| "inference resolution requires a scenario".to_string())?;
        let defaults = providers.defaults(&scenario)?;
        if defaults.model.trim().is_empty() {
            return Err(format!("scenario {scenario} resolved to an empty model"));
        }
        let effort = merged.effort.or(defaults.effort);
        Ok(Self {
            selection: Selection::Scenario(scenario),
            model: defaults.model,
            effort: effort.clone(),
            source: Source {
                route: None,
                selection: Selection::Scenario(
                    origin(layers, |declaration| declaration.scenario.is_some())
                        .unwrap_or(Origin::Default),
                ),
                model: Origin::Default,
                effort: origin(layers, |declaration| declaration.effort.is_some())
                    .or_else(|| effort.map(|_| Origin::Default)),
            },
        })
    }
}

fn origin(
    layers: &[(&Declaration, Origin)],
    present: impl Fn(&Declaration) -> bool,
) -> Option<Origin> {
    layers
        .iter()
        .rev()
        .find(|(declaration, _)| present(declaration))
        .map(|(_, origin)| *origin)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schema(as = inference::Reading)]
pub struct Reading {
    pub declaration: Declaration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective: Option<Effective>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schema(as = inference::Draft)]
pub struct Draft {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference: Option<Declaration>,
}
