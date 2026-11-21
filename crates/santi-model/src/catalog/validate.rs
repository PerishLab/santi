use super::{Definition, Kind, Record, selection};
use std::collections::BTreeSet;

fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn endpoint(value: &str) -> bool {
    let Ok(parsed) = url::Url::parse(value) else {
        return false;
    };
    let http = matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some();
    let private = !parsed.username().is_empty() || parsed.password().is_some();
    let suffix = parsed.query().is_some() || parsed.fragment().is_some();
    http && !private && !suffix
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 8192
}

impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        if self.name().is_some_and(|n| !name(n)) {
            return Err(
                "a source name requires letters, numbers, dots, hyphens or underscores".into(),
            );
        }
        let valid = match self {
            Self::Access {
                endpoint: url,
                credential,
                ..
            } => {
                let handle = credential.strip_prefix("custody://").is_some_and(name);
                endpoint(url) && handle
            }
            Self::Offering { model, .. } => {
                text(model) && model.trim() == model && !model.chars().any(char::is_control)
            }
            Self::Candidate { evidence, .. } => text(evidence),
            Self::Scenario { criterion, .. } => text(criterion),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(format!("invalid {} fields", self.kind().unit()))
        }
    }
}

pub fn graph(rows: &[Record]) -> Result<(), String> {
    let mut choices = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    let mut names = BTreeSet::new();
    for row in rows {
        for (kind, id) in row.definition.references() {
            selection::Directory(rows).find(kind, id)?;
        }
        match &row.definition {
            Definition::Candidate {
                offering, scenario, ..
            } if !pairs.insert((*offering, *scenario)) => {
                return Err("offering already recorded for this scenario".into());
            }
            Definition::Preference { candidate }
                if !choices.insert(selection::Directory(rows).scope(*candidate)?) =>
            {
                return Err(
                    "scenario already has a preferred candidate for this provider and family"
                        .into(),
                );
            }
            Definition::Access { name, .. } | Definition::Alias { name, .. }
                if !names.insert(name) =>
            {
                return Err("an access or alias already uses this name".into());
            }
            _ => (),
        }
    }
    Ok(())
}

pub fn dependents(rows: &[Record], kind: Kind, id: i64) -> Vec<String> {
    rows.iter()
        .filter(|r| r.definition.references().contains(&(kind, id)))
        .map(|r| format!("{}/{}", r.definition.kind().unit(), r.id))
        .collect()
}
