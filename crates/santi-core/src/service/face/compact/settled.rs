use crate::service::Service;

use super::slots::covered;

impl Service {
    pub(in crate::service::face) async fn settled(
        &self,
        strand: &str,
    ) -> Result<(String, String), String> {
        let spans = self.spans(strand).await?;
        let floor = spans
            .iter()
            .filter(|span| !covered(span, &spans))
            .map(|span| span.to)
            .max()
            .unwrap_or(0);
        let entries = self.store.entries(strand).await?;
        let loose = entries
            .iter()
            .filter(|entry| entry.kind == crate::strand::Target::Message)
            .map(|entry| entry.seq)
            .filter(|seq| *seq > floor)
            .collect::<Vec<_>>();
        let Some((_, settled)) = loose.split_last() else {
            return Err(unsettled(floor));
        };
        let (Some(first), Some(last)) = (settled.first(), settled.last()) else {
            return Err(unsettled(floor));
        };
        let first = self.seated(strand, *first).await?;
        let last = self.seated(strand, *last).await?;
        Ok((first, last))
    }

    async fn seated(&self, strand: &str, sequence: i64) -> Result<String, String> {
        self.store.seated(strand, sequence).await?.ok_or_else(|| {
            format!("compact boundary {sequence} is not a message sequence in this strand")
        })
    }
}

fn unsettled(floor: i64) -> String {
    format!(
        "compact has no settled range to collapse: every message after sequence {floor} is still the live exchange, and the most recent message is always kept out of a default range; name first/last or from/to to choose a range yourself"
    )
}
