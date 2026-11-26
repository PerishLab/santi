use crate::inference::{Declaration, Patch};
use anyhow::{Result, bail};
use clap::{Args, Subcommand, ValueEnum};

#[derive(Subcommand)]
pub enum Inference {
    #[command(
        about = "Inspect or manage inference sources and scenario preferences",
        subcommand
    )]
    Sources(Sources),
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
    #[arg(
        long,
        help = "Persist the provider/family/scenario path on the selected scope"
    )]
    pub scenario: Option<String>,
    #[arg(
        long = "reasoning-effort",
        value_name = "REASONING_EFFORT",
        help = "Persist this reasoning effort on the selected scope"
    )]
    pub effort: Option<String>,
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
    Scenario,
    #[value(name = "reasoning-effort", alias = "reasoning_effort")]
    Effort,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::Scenario => "scenario",
            Self::Effort => "reasoning_effort",
        }
    }
}

impl Edit {
    pub fn declaration(&self) -> Declaration {
        Declaration {
            scenario: self.scenario.clone(),
            effort: self.effort.clone(),
        }
    }

    pub fn patch(&self, held: &Declaration) -> Result<Patch> {
        for (field, explicit) in [
            (Field::Scenario, self.scenario.is_some()),
            (Field::Effort, self.effort.is_some()),
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
            scenario: pick(self.clear.contains(&Field::Scenario), &stated.scenario),
            effort: pick(self.clear.contains(&Field::Effort), &stated.effort),
        };
        let declared = Declaration {
            scenario: patch.scenario.clone().unwrap_or_default(),
            effort: patch.effort.clone().unwrap_or_default(),
        };
        declared.validate().map_err(anyhow::Error::msg)?;
        if patch.scenario.is_none() && patch.effort.is_none() {
            bail!("inference set requires a field value or --clear");
        }
        Ok(patch)
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct Delta {
    #[arg(
        long,
        help = "Declare the provider/family/scenario path on the new strand"
    )]
    pub scenario: Option<String>,
    #[arg(
        long = "reasoning-effort",
        value_name = "REASONING_EFFORT",
        help = "Declare this reasoning effort on the new strand"
    )]
    pub effort: Option<String>,
    #[arg(
        long,
        conflicts_with_all = ["scenario", "effort"],
        help = "Declare an explicit empty delta so every field inherits from the soul"
    )]
    pub inherit: bool,
}

impl Delta {
    pub fn declaration(&self) -> Declaration {
        Declaration {
            scenario: self.scenario.clone(),
            effort: self.effort.clone(),
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

#[derive(Subcommand)]
pub enum Sources {
    #[command(about = "List resource records with their stable estate IDs")]
    List,
    #[command(about = "Read one resource by kind and stable estate ID")]
    Get { kind: String, id: i64 },
    #[command(about = "Export the complete portable catalog as JSON on stdout (no estate IDs)")]
    Export,
    #[command(about = "Import a portable export, or legacy service TOML with --mapping")]
    Import(Import),
    #[command(about = "Explain the candidates and current preference for a scenario")]
    Explain {
        provider: String,
        family: String,
        scenario: String,
    },
    #[command(
        about = "Create a resource from a JSON definition; client operator setting selects the token file"
    )]
    Create { file: std::path::PathBuf },
    #[command(about = "Apply JSON containing expected record and replacement definition")]
    Update { file: std::path::PathBuf },
    #[command(about = "Retire the exact record supplied in the JSON file")]
    Delete { file: std::path::PathBuf },
}

#[derive(Args)]
pub struct Import {
    pub file: std::path::PathBuf,
    #[arg(
        long,
        help = "Operator TOML mapping (one [profiles.NAME] for each legacy provider)"
    )]
    pub mapping: Option<std::path::PathBuf>,
    #[arg(
        long = "dry-run",
        required_unless_present = "apply",
        conflicts_with = "apply",
        help = "Check coverage and preview creates against the current catalog; never write"
    )]
    pub preview: bool,
    #[arg(
        long,
        help = "Recheck coverage and create missing records; never replace or delete"
    )]
    pub apply: bool,
}
