use super::*;
use santi_model::receipt::{Acknowledgement, Confirmation};
use serde_json::Value;

impl Store {
    pub async fn acknowledge(
        &self,
        inbox: &str,
        intent: &Value,
        occurred: &str,
    ) -> Result<bool, String> {
        let intent = json(intent)?;
        self.core
            .batch(async |tx| {
                let receipt = read::need(tx, "InboxReceipt", "tag", inbox)
                    .await?
                    .to_string();
                if let Some(row) = tx
                    .one(&form("Acknowledgement").when("receipt", Op::Eq, &receipt))
                    .await?
                {
                    if row.text("intent") != Some(intent.as_str()) {
                        return Err(keel::adapt::Error::Adapt(
                            "acknowledgement intent conflicts with retained dispatch".into(),
                        ));
                    }
                    return Ok(false);
                }
                tx.put(
                    "Acknowledgement",
                    &[
                        ("receipt", &receipt),
                        ("intent", &intent),
                        ("state", "dispatching"),
                        ("created", occurred),
                        ("updated", occurred),
                    ],
                )
                .await?;
                Ok(true)
            })
            .await
            .map_err(read::error)
    }

    pub async fn acknowledged(
        &self,
        inbox: &str,
        result: &Value,
        occurred: &str,
    ) -> Result<(), String> {
        let state = result["state"]
            .as_str()
            .filter(|s| ["accepted", "rejected", "unsent", "unknown"].contains(s))
            .ok_or("invalid acknowledgement result state")?;
        let result = json(result)?;
        self.core
            .batch(async |tx| {
                let receipt = read::need(tx, "InboxReceipt", "tag", inbox)
                    .await?
                    .to_string();
                let row = tx
                    .one(&form("Acknowledgement").when("receipt", Op::Eq, &receipt))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing("acknowledgement".into()))?;
                if row.text("state") != Some("dispatching") {
                    return Err(keel::adapt::Error::Adapt(
                        "acknowledgement already has a result; no replay".into(),
                    ));
                }
                tx.set(
                    "Acknowledgement",
                    row.key(),
                    &[("state", state), ("result", &result), ("updated", occurred)],
                )
                .await
            })
            .await
            .map_err(read::error)
    }

    pub async fn interrupted(&self, occurred: &str) -> Result<(), String> {
        self.core
            .batch(async |tx| {
                let rows = tx
                    .ask(&form("Acknowledgement").when("state", Op::Eq, "dispatching"))
                    .await?;
                for row in rows.rows() {
                    tx.set(
                        "Acknowledgement",
                        row.key(),
                        &[("state", "unknown"), ("updated", occurred)],
                    )
                    .await?;
                }
                Ok(())
            })
            .await
            .map_err(read::error)
    }

    pub async fn confirmed(
        &self,
        inbox: &str,
        resolution: &Confirmation,
        occurred: &str,
    ) -> Result<(), String> {
        if resolution.evidence.trim().is_empty() {
            return Err("acknowledgement resolution requires exact observation evidence".into());
        }
        let encoded = json(resolution)?;
        self.core
            .batch(async |tx| {
                let receipt = read::need(tx, "InboxReceipt", "tag", inbox)
                    .await?
                    .to_string();
                let row = tx
                    .one(&form("Acknowledgement").when("receipt", Op::Eq, &receipt))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing("acknowledgement".into()))?;
                if row.text("state") != Some("unknown") {
                    return Err(keel::adapt::Error::Adapt(
                        "only an unknown acknowledgement can be explicitly resolved".into(),
                    ));
                }
                let state = if resolution.applied {
                    "observed"
                } else {
                    "not_applied"
                };
                tx.set(
                    "Acknowledgement",
                    row.key(),
                    &[
                        ("state", state),
                        ("resolution", &encoded),
                        ("updated", occurred),
                    ],
                )
                .await
            })
            .await
            .map_err(read::error)
    }

    pub async fn acknowledgement(&self, inbox: &str) -> Result<Option<Acknowledgement>, String> {
        let Some(receipt) = read::one(&self.core, "InboxReceipt", "tag", inbox).await? else {
            return Ok(None);
        };
        let Some(row) = read::one(
            &self.core,
            "Acknowledgement",
            "receipt",
            &receipt.key().to_string(),
        )
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(Acknowledgement {
            inbox: inbox.into(),
            intent: serde_json::from_str(read::text(&row, "intent")?).map_err(|e| e.to_string())?,
            state: read::text(&row, "state")?.to_string(),
            resolution: row
                .text("resolution")
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| e.to_string())?,
            result: row
                .text("result")
                .map(serde_json::from_str)
                .transpose()
                .map_err(|e| e.to_string())?,
            created: read::text(&row, "created")?.to_string(),
            updated: read::text(&row, "updated")?.to_string(),
        }))
    }
}
