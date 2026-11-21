use super::{
    Definition, Kind, Protocol,
    selection::{Choice, Directory},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Alternative {
    pub candidate: i64,
    pub offering: i64,
    pub access: String,
    pub model: String,
    pub protocol: Protocol,
    pub evidence: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Explanation {
    #[serde(flatten)]
    pub choice: Choice,
    pub criterion: String,
    pub options: Vec<Alternative>,
}

impl Directory<'_> {
    pub fn describe(
        &self,
        provider: &str,
        family: &str,
        scenario: &str,
    ) -> Result<Explanation, String> {
        let choice = self.explain(provider, family, scenario)?;
        let Definition::Scenario { criterion, .. } = self.find(Kind::Scenario, choice.scenario)?
        else {
            unreachable!()
        };
        let options = choice
            .candidates
            .iter()
            .map(|id| self.alternative(*id))
            .collect::<Result<_, _>>()?;
        Ok(Explanation {
            choice,
            criterion: criterion.clone(),
            options,
        })
    }

    fn alternative(&self, candidate: i64) -> Result<Alternative, String> {
        let Definition::Candidate {
            offering, evidence, ..
        } = self.find(Kind::Candidate, candidate)?
        else {
            unreachable!()
        };
        let Definition::Offering { access, model, .. } = self.find(Kind::Offering, *offering)?
        else {
            unreachable!()
        };
        let Definition::Access { name, protocol, .. } = self.find(Kind::Access, *access)? else {
            unreachable!()
        };
        Ok(Alternative {
            candidate,
            offering: *offering,
            access: name.clone(),
            model: model.clone(),
            protocol: *protocol,
            evidence: evidence.clone(),
        })
    }
}
