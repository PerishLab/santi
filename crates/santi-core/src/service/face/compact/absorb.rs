use crate::service::Service;

use super::slots::covered;

impl Service {
    pub(in crate::service::face) async fn merged(
        &self,
        strand: &str,
        named: &[String],
    ) -> Result<(String, String), String> {
        let spans = self.spans(strand).await?;
        let live = spans
            .iter()
            .filter(|span| !covered(span, &spans))
            .collect::<Vec<_>>();
        let mut chosen = Vec::new();
        for name in named {
            let found = live
                .iter()
                .find(|span| span.compact == *name)
                .ok_or_else(|| stranger(name))?;
            chosen.push((*found).clone());
        }
        chosen.sort_by_key(|span| span.from);
        let missed = live
            .iter()
            .filter(|span| between(span, &chosen))
            .map(|span| span.compact.clone())
            .collect::<Vec<_>>();
        if !missed.is_empty() {
            return Err(gapped(&missed));
        }
        let first = chosen
            .first()
            .ok_or_else(|| "absorb names no slot".to_string())?;
        let last = chosen
            .last()
            .ok_or_else(|| "absorb names no slot".to_string())?;
        Ok((first.head.clone(), last.tail.clone()))
    }
}

fn between(span: &super::slots::Span, chosen: &[super::slots::Span]) -> bool {
    let Some(low) = chosen.first() else {
        return false;
    };
    let Some(high) = chosen.last() else {
        return false;
    };
    span.from > low.from
        && span.to < high.to
        && !chosen.iter().any(|held| held.compact == span.compact)
}

fn stranger(name: &str) -> String {
    format!(
        "absorb names {name}, which is not an occupied slot on this strand; read the slot occupancy beside the budget for the ids that are"
    )
}

fn gapped(missed: &[String]) -> String {
    format!(
        "absorb names a set that is not contiguous: {} lies between the slots you named and would be collapsed without being chosen; name it too, or choose adjacent slots",
        missed.join(", ")
    )
}
