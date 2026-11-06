use super::{Store, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Row, form};
use santi_model::receipt::reply::{Attempt, Binding, Owner, Resolution, Status, Window};
use serde_json::Value;
mod state;
use state::Change;
mod gate;
mod input;
pub(in crate::store) use input::attach;

pub(in crate::store) async fn create(
    tx: &mut keel::Tx<'_, Sqlite>,
    inbox: &str,
    binding: &Binding,
) -> Result<(), Error> {
    let receipt = read::need(tx, "InboxReceipt", "tag", inbox).await?;
    let record = Window {
        binding: binding.clone(),
        owner: Owner::Runtime,
        message: None,
        attempts: Vec::new(),
    };
    let record = serde_json::to_string(&record).map_err(|e| Error::Adapt(e.to_string()))?;
    tx.put(
        "Window",
        &[("receipt", &receipt.to_string()), ("record", &record)],
    )
    .await?;
    Ok(())
}

impl Store {
    pub async fn window(&self, inbox: &str) -> Result<Option<Status>, String> {
        let Some(receipt) = read::one(&self.core, "InboxReceipt", "tag", inbox).await? else {
            return Ok(None);
        };
        let Some(row) =
            read::one(&self.core, "Window", "receipt", &receipt.key().to_string()).await?
        else {
            return Ok(None);
        };
        self.status(&row, &receipt).await.map(Some)
    }

    pub async fn replies(&self) -> Result<Vec<Status>, String> {
        let rows = self.core.ask(&form("Window")).await.map_err(read::error)?;
        let mut statuses = Vec::new();
        for row in rows.rows() {
            let receipt = read::one(
                &self.core,
                "InboxReceipt",
                "id",
                &read::int(row, "receipt")?.to_string(),
            )
            .await?
            .ok_or("reply receipt missing")?;
            statuses.push(self.status(row, &receipt).await?);
        }
        Ok(statuses)
    }

    async fn status(&self, row: &Row, receipt: &Row) -> Result<Status, String> {
        Ok(Status {
            inbox: read::text(receipt, "tag")?.into(),
            strand: read::related(&self.core, "Strand", read::int(receipt, "strand")?).await?,
            accepted: read::text(receipt, "accepted")?.into(),
            window: decode(row)?,
        })
    }

    pub async fn queue(&self, inbox: &str, intent: Intent<'_>) -> Result<(), String> {
        self.revise(inbox, Change::Queue(intent)).await?;
        Ok(())
    }

    pub async fn dispatch(&self, inbox: &str) -> Result<Option<Attempt>, String> {
        self.revise(inbox, Change::Dispatch).await
    }

    pub async fn delivered(&self, inbox: &str, result: Outcome<'_>) -> Result<(), String> {
        self.revise(inbox, Change::Settle(result)).await?;
        Ok(())
    }

    pub async fn resolution(&self, inbox: &str, resolution: &Resolution) -> Result<(), String> {
        self.revise(inbox, Change::Resolve(resolution)).await?;
        Ok(())
    }

    pub async fn interrupt(&self) -> Result<(), String> {
        for reply in self.replies().await? {
            self.revise(&reply.inbox, Change::Interrupt).await?;
        }
        Ok(())
    }

    async fn revise(&self, inbox: &str, change: Change<'_>) -> Result<Option<Attempt>, String> {
        self.core
            .batch(async |tx| {
                let receipt = read::need(tx, "InboxReceipt", "tag", inbox).await?;
                let row = tx
                    .one(&form("Window").when("receipt", Op::Eq, &receipt.to_string()))
                    .await?
                    .ok_or_else(|| Error::Missing("reply window".into()))?;
                let mut window = decode(&row).map_err(Error::Adapt)?;
                gate::check(tx, receipt, &mut window, &change).await?;
                let dispatch = state::Machine(&mut window)
                    .apply(change)
                    .map_err(Error::Adapt)?;
                let record =
                    serde_json::to_string(&window).map_err(|e| Error::Adapt(e.to_string()))?;
                if row.text("record") != Some(record.as_str()) {
                    tx.set("Window", row.key(), &[("record", &record)]).await?;
                }
                Ok(dispatch)
            })
            .await
            .map_err(read::error)
    }
}

pub struct Intent<'a> {
    pub owner: Owner,
    pub operation: &'a str,
    pub body: &'a Value,
}

pub struct Outcome<'a> {
    pub operation: &'a str,
    pub result: &'a Value,
    pub message: Option<&'a str>,
}

fn decode(row: &Row) -> Result<Window, String> {
    serde_json::from_str(read::text(row, "record")?).map_err(|e| e.to_string())
}

pub(in crate::store) async fn retire(
    tx: &mut keel::Tx<'_, Sqlite>,
    strand: &str,
) -> Result<(), Error> {
    use santi_model::receipt::reply::State;
    let receipts = tx
        .ask(&form("InboxReceipt").when("strand", Op::Eq, strand))
        .await?;
    for receipt in receipts.rows() {
        if let Some(row) = tx
            .one(&form("Window").when("receipt", Op::Eq, &receipt.key().to_string()))
            .await?
        {
            let window = decode(&row).map_err(Error::Adapt)?;
            if window
                .attempts
                .iter()
                .any(|a| matches!(a.state, State::Queued | State::Dispatching | State::Unknown))
            {
                return Err(Error::Adapt(
                    "unsettled channel reply blocks strand retirement".into(),
                ));
            }
        }
    }
    Ok(())
}
