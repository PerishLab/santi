use super::{Definition, Kind, Record};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Choice {
    pub provider: i64,
    pub family: i64,
    pub scenario: i64,
    pub candidates: Vec<i64>,
    pub preferred: Option<i64>,
}

pub struct Directory<'a>(pub &'a [Record]);

impl<'a> Directory<'a> {
    pub fn find(&self, kind: Kind, id: i64) -> Result<&'a Definition, String> {
        self.0
            .iter()
            .find(|r| r.id == id && r.definition.kind() == kind)
            .map(|r| &r.definition)
            .ok_or_else(|| format!("missing {} {id}", kind.unit()))
    }

    pub fn scope(&self, candidate: i64) -> Result<(i64, i64, i64), String> {
        let Definition::Candidate {
            offering, scenario, ..
        } = self.find(Kind::Candidate, candidate)?
        else {
            unreachable!()
        };
        let Definition::Offering { access, family, .. } = self.find(Kind::Offering, *offering)?
        else {
            unreachable!()
        };
        let Definition::Access { provider, .. } = self.find(Kind::Access, *access)? else {
            unreachable!()
        };
        Ok((*provider, *family, *scenario))
    }

    fn named(&self, kind: Kind, name: &str) -> Result<i64, String> {
        self.0
            .iter()
            .find(|row| row.definition.kind() == kind && row.definition.name() == Some(name))
            .map(|row| row.id)
            .ok_or_else(|| format!("unknown {} {name}", kind.unit()))
    }

    pub fn explain(&self, provider: &str, family: &str, scenario: &str) -> Result<Choice, String> {
        if !santi::inference::word(provider)
            || !santi::inference::word(family)
            || !santi::inference::scenario(scenario)
        {
            return Err("invalid provider/family/scenario path".into());
        }
        let provider = self.named(Kind::Provider, provider)?;
        let family = self.named(Kind::Family, family)?;
        let scenario = self.named(Kind::Scenario, scenario)?;
        let mut candidates = Vec::new();
        for row in self
            .0
            .iter()
            .filter(|r| r.definition.kind() == Kind::Candidate)
        {
            if self.scope(row.id)? == (provider, family, scenario) {
                candidates.push(row.id);
            }
        }
        let mut preferred = None;
        for row in self.0 {
            if let Definition::Preference { candidate } = row.definition
                && candidates.contains(&candidate)
                && preferred.replace(candidate).is_some()
            {
                return Err("several preferences for one scenario selection".into());
            }
        }
        Ok(Choice {
            provider,
            family,
            scenario,
            candidates,
            preferred,
        })
    }
}
