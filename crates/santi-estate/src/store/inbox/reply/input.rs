use super::{Store, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, form};
use std::collections::BTreeMap;

pub(in crate::store) async fn attach(
    tx: &mut keel::Tx<'_, Sqlite>,
    inbox: &str,
    input: i64,
) -> Result<(), Error> {
    let receipt = read::need(tx, "InboxReceipt", "tag", inbox).await?;
    let Some(window) = tx
        .one(&form("Window").when("receipt", Op::Eq, &receipt.to_string()))
        .await?
    else {
        return Ok(());
    };
    if window.int("input").is_some_and(|old| old != input) {
        return Err(Error::Adapt(
            "reply input conflicts with its receipt".into(),
        ));
    }
    tx.set("Window", window.key(), &[("input", &input.to_string())])
        .await
}

impl Store {
    pub async fn receptions(&self, strand: &str) -> Result<BTreeMap<String, String>, String> {
        let strand = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or("strand not found")?;
        let rows = self
            .core
            .ask(&form("InboxReceipt").when("strand", Op::Eq, &strand.key().to_string()))
            .await
            .map_err(read::error)?;
        let receipts = rows
            .rows()
            .iter()
            .map(|row| Ok((row.key(), read::text(row, "tag")?.to_string())))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let keys = receipts.keys().map(i64::to_string).collect::<Vec<_>>();
        let mut inputs = BTreeMap::new();
        for chunk in keys.chunks(128) {
            let keys = chunk.iter().map(String::as_str).collect::<Vec<_>>();
            let rows = self
                .core
                .ask(&form("Window").any("receipt", &keys))
                .await
                .map_err(read::error)?;
            inputs.extend(
                rows.rows()
                    .iter()
                    .map(|row| {
                        let Some(input) = row.int("input") else {
                            return Ok(None);
                        };
                        let receipt = read::int(row, "receipt")?;
                        let inbox = receipts
                            .get(&receipt)
                            .ok_or("reply receipt relation missing")?;
                        Ok(Some((input, inbox.clone())))
                    })
                    .collect::<Result<Vec<_>, String>>()?
                    .into_iter()
                    .flatten(),
            );
        }
        let keys = inputs.keys().map(i64::to_string).collect::<Vec<_>>();
        let mut result = BTreeMap::new();
        for chunk in keys.chunks(128) {
            let keys = chunk.iter().map(String::as_str).collect::<Vec<_>>();
            let rows = self
                .core
                .ask(&form("Message").any("id", &keys))
                .await
                .map_err(read::error)?;
            result.extend(
                rows.rows()
                    .iter()
                    .map(|row| {
                        let message = read::text(row, "tag")?.to_string();
                        let inbox = inputs
                            .get(&row.key())
                            .ok_or("reply input relation missing")?
                            .clone();
                        Ok((message, inbox))
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            );
        }
        if result.len() != inputs.len() {
            return Err("reply input relation incomplete".into());
        }
        Ok(result)
    }
}
