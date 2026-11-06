use super::*;
use crate::context::budget::gauged;
use santi_provider::Provider;

impl Service {
    pub(super) async fn witnessed(
        &self,
        turn: &str,
        round: usize,
        ids: Vec<String>,
        input: &[santi_provider::Item],
    ) -> Result<(), String> {
        self.store
            .prepared(turn, round, &selected(ids, input), &crate::now())
            .await
    }

    pub(in crate::service) fn eligible(&self, strand: &str, turn: &str, round: usize) -> bool {
        round > 1
            && self
                .rationed(strand)
                .is_none_or(|budget| round <= budget.rounds)
            && self.settlement(turn).is_none()
    }

    pub(super) async fn absorb(
        &self,
        strand: &str,
        turn: &str,
        provider: &dyn Provider,
        control: &Control,
    ) -> Result<Vec<String>, String> {
        let Some(intake) = self.store.intake(strand, turn, &crate::now()).await? else {
            return Ok(Vec::new());
        };
        let input = self.assembly(strand, turn, intake.messages()).await?;
        let composition = self.composition(strand).await?;
        let tools = self.offers(strand, Some(turn)).await?;
        let request = match self.projected(provider, input, composition.text, tools) {
            Ok(request) => request,
            Err(_) => return Ok(Vec::new()),
        };
        if provider.validate(&request).is_err() {
            return Ok(Vec::new());
        }
        if let Some(cap) = self.budget(provider)
            && gauged(&request, provider).total > cap.bytes
        {
            return Ok(Vec::new());
        }
        if self.halted(control).is_some() || !self.store.absorb(&intake).await? {
            return Ok(Vec::new());
        }
        Ok(intake
            .messages()
            .iter()
            .map(|m| m.message.id.clone())
            .collect())
    }
}

pub(super) fn selected(ids: Vec<String>, input: &[santi_provider::Item]) -> Vec<String> {
    ids.into_iter()
        .filter(|id| {
            let header = format!("[message {id}]\n");
            input.iter().any(|item| {
                matches!(item, santi_provider::Item::Message { role, content }
            if matches!(role.as_str(), "user" | "system")
                && content.strip_prefix(&header).is_some_and(|text| !text.trim().is_empty()))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::selected;
    use santi_provider::Item;

    #[test]
    fn identity() {
        let item = |role: &str, text: &str| Item::Message {
            role: role.into(),
            content: text.into(),
        };
        let ids = || vec!["new".into()];
        assert!(selected(ids(), &[]).is_empty());
        assert!(
            selected(
                ids(),
                &[item("user", "[message old]\nquoted [message new]\ntext")]
            )
            .is_empty()
        );
        assert!(selected(ids(), &[item("assistant", "[message new]\ntext")]).is_empty());
        assert!(selected(ids(), &[item("user", "[message new]\n \n")]).is_empty());
        assert_eq!(
            selected(ids(), &[item("user", "[message new]\ntext")]),
            vec!["new"]
        );
        assert_eq!(
            selected(ids(), &[item("system", "[message new]\nnotice")]),
            vec!["new"]
        );
    }
}
