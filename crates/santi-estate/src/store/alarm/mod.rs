use super::{Store, read};
use keel::{Op, Row, form};
use santi_model::alarm;

mod accept;
mod change;

impl Store {
    pub async fn alarms(&self) -> Result<Vec<alarm::Alarm>, String> {
        let rows = self.core.ask(&form("Alarm")).await.map_err(read::error)?;
        let mut alarms = Vec::new();
        for row in rows.rows() {
            alarms.push(self.alarm_row(row).await?);
        }
        alarms.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(alarms)
    }

    pub async fn alarm(&self, tag: &str) -> Result<Option<alarm::Alarm>, String> {
        match read::one(&self.core, "Alarm", "tag", tag).await? {
            Some(row) => self.alarm_row(&row).await.map(Some),
            None => Ok(None),
        }
    }

    async fn alarm_row(&self, row: &Row) -> Result<alarm::Alarm, String> {
        let strand = read::related(&self.core, "Strand", read::int(row, "strand")?).await?;
        let owner = self.strand(&strand).await?.ok_or("alarm strand missing")?;
        Ok(alarm::Alarm {
            id: read::text(row, "tag")?.into(),
            name: read::text(row, "name")?.into(),
            soul: owner.soul,
            strand,
            text: read::text(row, "text")?.into(),
            cron: read::text(row, "cron")?.into(),
            generation: read::int(row, "generation")?,
        })
    }
}
