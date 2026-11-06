pub mod inference {
    use radicula_config::Cascade;
    use serde::{Deserialize, Deserializer, Serialize};
    use utoipa::ToSchema;

    #[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq, Cascade)]
    #[cascade(strict)]
    #[serde(deny_unknown_fields)]
    #[schema(as = inference::Declaration)]
    pub struct Declaration {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub provider: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reasoning_effort: Option<String>,
    }

    impl Declaration {
        pub fn empty(&self) -> bool {
            self.provider.is_none() && self.model.is_none() && self.reasoning_effort.is_none()
        }

        pub fn partial(&self) -> DeclarationPartial {
            DeclarationPartial {
                provider: self.provider.clone().map(Some),
                model: self.model.clone().map(Some),
                reasoning_effort: self.reasoning_effort.clone().map(Some),
            }
        }

        pub fn overlay(&self, lower: &Self) -> Self {
            lower.clone().merge(self.partial())
        }

        pub fn validate(&self) -> Result<(), String> {
            for (field, value) in [
                ("provider", &self.provider),
                ("model", &self.model),
                ("reasoning_effort", &self.reasoning_effort),
            ] {
                if let Some(value) = value
                    && value.trim().is_empty()
                {
                    return Err(format!("inference {field} must not be empty"));
                }
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
        pub provider: Option<Option<String>>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "nullable"
        )]
        pub model: Option<Option<String>>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "nullable"
        )]
        pub reasoning_effort: Option<Option<String>>,
    }

    impl Patch {
        pub fn apply(&self, base: &Declaration) -> Declaration {
            base.clone().merge(DeclarationPartial {
                provider: self.provider.clone(),
                model: self.model.clone(),
                reasoning_effort: self.reasoning_effort.clone(),
            })
        }

        pub fn touches(&self, field: &str) -> bool {
            match field {
                "provider" => self.provider.is_some(),
                "model" => self.model.is_some(),
                "reasoning_effort" => self.reasoning_effort.is_some(),
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
        Service,
        Soul,
        Strand,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
    #[schema(as = inference::Source)]
    pub struct Source {
        pub provider: Origin,
        pub model: Origin,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reasoning_effort: Option<Origin>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Defaults {
        pub model: String,
        pub reasoning_effort: Option<String>,
    }

    pub trait Directory {
        fn defaults(&self, provider: &str) -> Result<Defaults, String>;
    }

    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
    #[schema(as = inference::Effective)]
    pub struct Effective {
        pub provider: String,
        pub model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reasoning_effort: Option<String>,
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
            let provider = merged
                .provider
                .clone()
                .ok_or_else(|| "inference resolution requires a provider".to_string())?;
            let defaults = providers.defaults(&provider)?;
            if defaults.model.trim().is_empty() {
                return Err(format!("provider {provider} has an empty default model"));
            }
            let model = merged.model.unwrap_or(defaults.model);
            let reasoning_effort = merged.reasoning_effort.or(defaults.reasoning_effort);
            Ok(Self {
                provider: provider.clone(),
                model,
                reasoning_effort: reasoning_effort.clone(),
                source: Source {
                    provider: origin(layers, |declaration| declaration.provider.is_some())
                        .unwrap_or(Origin::Default),
                    model: origin(layers, |declaration| declaration.model.is_some())
                        .unwrap_or(Origin::Default),
                    reasoning_effort: origin(layers, |declaration| {
                        declaration.reasoning_effort.is_some()
                    })
                    .or_else(|| reasoning_effort.map(|_| Origin::Default)),
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
}

#[cfg(feature = "client")]
pub mod auth;
#[cfg(feature = "client")]
pub mod cli;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub mod config;
#[cfg(feature = "client")]
#[path = "cli/observation.rs"]
mod observation;
#[cfg(feature = "client")]
mod text;
#[cfg(feature = "client")]
pub mod watch;

#[cfg(feature = "client")]
use anyhow::Result;
#[cfg(feature = "client")]
use clap::Parser;

#[cfg(feature = "client")]
use auth::{Credentials, resolve_edge_bearer};
#[cfg(feature = "client")]
use cli::{BASE, Cli, ClientDefaults, Command, Inference, Scope, StrandCommand};
#[cfg(feature = "client")]
use config::ClientPartial;
#[cfg(feature = "client")]
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let observation = observation::start(&cli.command);
    let result = execute(cli).await;
    if let Some(observation) = observation {
        observation.finish(if result.is_ok() { 0 } else { 1 });
    }
    result
}

#[cfg(feature = "client")]
#[locus::trace(with = plumb::observation::view())]
async fn execute(cli: Cli) -> Result<()> {
    let Cli {
        base_url,
        api_key,
        auth_token_url,
        auth_client_id,
        auth_username,
        auth_password,
        strand,
        soul,
        command,
    } = cli;
    plumb::identity::ready().map_err(anyhow::Error::msg)?;
    if let Command::Skill(deed) = command {
        return cli::skill::run(deed);
    }
    let stated = match &command {
        Command::Inference(Inference::Soul(Scope::Set(edit)))
        | Command::Inference(Inference::Strand(Scope::Set(edit))) => edit.declaration(),
        Command::Strand(StrandCommand::Create(delta))
        | Command::Strand(StrandCommand::Fork { delta, .. }) => delta.declaration(),
        _ => inference::Declaration::default(),
    };
    let client = config::client(ClientPartial {
        base_url: base_url.map(Some),
        api_key: api_key.map(Some),
        auth_token_url: auth_token_url.map(Some),
        auth_client_id: auth_client_id.map(Some),
        auth_username: auth_username.map(Some),
        auth_password: auth_password.map(Some),
        token_cache: None,
        inference: stated.partial(),
    })?;
    let held = client.inference.clone();
    let defaults = ClientDefaults { strand, soul, held };
    let bearer = resolve_edge_bearer(Credentials {
        endpoint: client.auth_token_url.as_deref(),
        identity: client.auth_client_id.as_deref(),
        username: client.auth_username.as_deref(),
        password: client.auth_password.as_deref(),
        key: client.api_key.as_deref(),
        cache: client.token_cache.as_deref(),
    })
    .await?;
    let base = client.base_url.unwrap_or_else(|| BASE.to_owned());
    client::run(&base, bearer.as_deref(), &defaults, command).await
}
