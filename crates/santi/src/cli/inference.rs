use crate::inference::{Declaration, Patch};
use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Subcommand)]
pub enum Inference {
    #[command(about = "Manage the inference declaration of one soul", subcommand)]
    Soul(Scope),
    #[command(about = "Manage the inference declaration of one strand", subcommand)]
    Strand(Scope),
}

#[derive(Subcommand)]
pub enum Scope {
    #[command(about = "Read the persisted declaration, effective selection and sources")]
    Get {
        #[arg(
            help = "Explicit target id, otherwise --soul/--strand or SANTI_SOUL_ID/SANTI_STRAND_ID"
        )]
        id: Option<String>,
    },
    #[command(
        about = "Persist selected fields; omit a field to keep it and clear to inherit again"
    )]
    Set(Edit),
}

#[derive(Args, Clone, Debug, Default)]
pub struct Edit {
    #[arg(help = "Explicit target id, otherwise --soul/--strand or SANTI_SOUL_ID/SANTI_STRAND_ID")]
    pub id: Option<String>,
    #[arg(long, help = "Persist this provider reference on the selected scope")]
    pub provider: Option<String>,
    #[arg(long, help = "Persist this model on the selected scope")]
    pub model: Option<String>,
    #[arg(long, help = "Persist this reasoning effort on the selected scope")]
    pub reasoning_effort: Option<String>,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        help = "Clear this persisted field so it inherits again; repeat or comma-separate"
    )]
    pub clear: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Field {
    Provider,
    Model,
    #[value(name = "reasoning-effort", alias = "reasoning_effort")]
    Effort,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Model => "model",
            Self::Effort => "reasoning_effort",
        }
    }
}

impl Edit {
    pub fn declaration(&self) -> Declaration {
        Declaration {
            provider: self.provider.clone(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
        }
    }

    pub fn patch(&self, held: &Declaration) -> Result<Patch> {
        for (field, explicit) in [
            (Field::Provider, self.provider.is_some()),
            (Field::Model, self.model.is_some()),
            (Field::Effort, self.reasoning_effort.is_some()),
        ] {
            if explicit && self.clear.contains(&field) {
                bail!(
                    "inference field {} cannot be both set and cleared",
                    field.name()
                );
            }
        }
        let stated = held.overlay(&self.declaration());
        let patch = Patch {
            provider: pick(self.clear.contains(&Field::Provider), &stated.provider),
            model: pick(self.clear.contains(&Field::Model), &stated.model),
            reasoning_effort: pick(
                self.clear.contains(&Field::Effort),
                &stated.reasoning_effort,
            ),
        };
        let declared = Declaration {
            provider: patch.provider.clone().unwrap_or_default(),
            model: patch.model.clone().unwrap_or_default(),
            reasoning_effort: patch.reasoning_effort.clone().unwrap_or_default(),
        };
        declared.validate().map_err(anyhow::Error::msg)?;
        if patch.provider.is_none() && patch.model.is_none() && patch.reasoning_effort.is_none() {
            bail!("inference set requires a field value or --clear");
        }
        Ok(patch)
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct Delta {
    #[arg(long, help = "Declare this provider reference on the new strand")]
    pub provider: Option<String>,
    #[arg(long, help = "Declare this model on the new strand")]
    pub model: Option<String>,
    #[arg(long, help = "Declare this reasoning effort on the new strand")]
    pub reasoning_effort: Option<String>,
    #[arg(
        long,
        conflicts_with_all = ["provider", "model", "reasoning_effort"],
        help = "Declare an explicit empty delta so every field inherits from the soul"
    )]
    pub inherit: bool,
}

impl Delta {
    pub fn declaration(&self) -> Declaration {
        Declaration {
            provider: self.provider.clone(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
        }
    }

    pub fn body(&self, held: &Declaration) -> Result<Option<serde_json::Value>> {
        if self.inherit {
            if !self.declaration().empty() {
                bail!("inference --inherit cannot be combined with field values");
            }
            return Ok(Some(serde_json::json!({ "inference": {} })));
        }
        let stated = held.overlay(&self.declaration());
        stated.validate().map_err(anyhow::Error::msg)?;
        if stated.empty() {
            Ok(None)
        } else {
            Ok(Some(serde_json::json!({ "inference": stated })))
        }
    }
}

fn pick(clear: bool, held: &Option<String>) -> Option<Option<String>> {
    if clear {
        Some(None)
    } else {
        held.clone().map(Some)
    }
}
