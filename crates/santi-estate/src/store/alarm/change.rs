use super::*;

impl Store {
    pub async fn set_alarm(&self, draft: &alarm::Draft) -> Result<alarm::Alarm, String> {
        let owner = self
            .strand(&draft.strand)
            .await?
            .ok_or("strand not found")?;
        if owner.soul != draft.soul {
            return Err("alarm strand belongs to another soul".into());
        }
        let proposed = santi_model::tag("alarm");
        let tag = self
            .core
            .batch(async |tx| {
                let strand = read::need(tx, "Strand", "tag", &draft.strand).await?;
                let key = strand.to_string();
                let old = tx
                    .one(&form("Alarm").when("strand", Op::Eq, &key).when(
                        "name",
                        Op::Eq,
                        &draft.name,
                    ))
                    .await?;
                let Some(old) = old else {
                    if draft.generation.is_some() {
                        return Err(keel::adapt::Error::Adapt(
                            "alarm generation conflicts with absent declaration".into(),
                        ));
                    }
                    tx.put(
                        "Alarm",
                        &[
                            ("tag", &proposed),
                            ("name", &draft.name),
                            ("text", &draft.text),
                            ("cron", &draft.cron),
                            ("generation", "1"),
                            ("strand", &key),
                        ],
                    )
                    .await?;
                    return Ok(proposed.clone());
                };
                let tag = old
                    .text("tag")
                    .ok_or_else(|| keel::adapt::Error::Adapt("alarm tag missing".into()))?
                    .to_string();
                let generation = old
                    .int("generation")
                    .ok_or_else(|| keel::adapt::Error::Adapt("alarm generation missing".into()))?;
                if draft
                    .generation
                    .is_some_and(|expected| expected != generation)
                {
                    return Err(keel::adapt::Error::Adapt(
                        "alarm generation conflicts; read the current declaration before updating"
                            .into(),
                    ));
                }
                if old.text("text") == Some(draft.text.as_str())
                    && old.text("cron") == Some(draft.cron.as_str())
                {
                    return Ok(tag);
                }
                if draft.generation != Some(generation) {
                    return Err(keel::adapt::Error::Adapt(
                        "alarm generation conflicts; read the current declaration before updating"
                            .into(),
                    ));
                }
                let next = generation
                    .checked_add(1)
                    .ok_or_else(|| keel::adapt::Error::Adapt("alarm generation exhausted".into()))?
                    .to_string();
                tx.set(
                    "Alarm",
                    old.key(),
                    &[
                        ("text", &draft.text),
                        ("cron", &draft.cron),
                        ("generation", &next),
                    ],
                )
                .await?;
                Ok(tag)
            })
            .await
            .map_err(read::error)?;
        self.alarm(&tag)
            .await?
            .ok_or("written alarm missing".into())
    }

    pub async fn end_alarm(&self, tag: &str, soul: &str) -> Result<(), String> {
        let Some(alarm) = self.alarm(tag).await? else {
            return Ok(());
        };
        if alarm.soul != soul {
            return Err("alarm belongs to another soul".into());
        }
        self.core
            .batch(async |tx| {
                if let Some(row) = tx.one(&form("Alarm").when("tag", Op::Eq, tag)).await? {
                    tx.end("Alarm", row.key()).await?;
                }
                Ok(())
            })
            .await
            .map_err(read::error)
    }
}
