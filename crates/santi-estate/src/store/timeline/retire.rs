use super::{Store, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Rank, Row, Tx, form};
use santi_model::strand::Retirement;
use std::collections::BTreeSet;

impl Store {
    pub async fn retire(&self, tag: &str) -> Result<(Retirement, Vec<String>), String> {
        self.core
            .batch(async |tx| {
                let row = tx
                    .one(&form("Strand").when("tag", Op::Eq, tag))
                    .await?
                    .ok_or_else(|| Error::Missing(format!("Strand {tag}")))?;
                let strand = row.key().to_string();
                Gate(tx).forks(&strand, tag).await?;
                Gate(tx).inputs(&strand).await?;
                let turns = Gate(tx).turns(&strand).await?;
                Gate(tx).acknowledgements(&strand).await?;
                super::super::inbox::reply::retire(tx, &strand).await?;
                let jobs = Gate(tx).jobs(&strand).await?;
                let retirement = record(tx, &row, turns, &jobs).await?;
                tx.end("Strand", row.key()).await?;
                let files = jobs
                    .rows()
                    .iter()
                    .flat_map(|row| [row.text("tag"), row.text("generation")])
                    .flatten()
                    .map(str::to_string)
                    .collect();
                Ok((retirement, files))
            })
            .await
            .map_err(read::error)
    }

    pub async fn retirement(&self, tag: &str) -> Result<Option<Retirement>, String> {
        read::one(&self.core, "Retirement", "tag", tag)
            .await?
            .map(|row| {
                Ok(Retirement {
                    id: read::text(&row, "tag")?.into(),
                    ended: read::text(&row, "ended")?.into(),
                    jobs: serde_json::from_str(read::text(&row, "jobs")?)
                        .map_err(|error| error.to_string())?,
                    messages: count(&row, "messages")?,
                    turns: count(&row, "turns")?,
                })
            })
            .transpose()
    }
}

fn count(row: &Row, field: &str) -> Result<usize, String> {
    read::int(row, field)?
        .try_into()
        .map_err(|error: std::num::TryFromIntError| error.to_string())
}

async fn record(
    tx: &mut Tx<'_, Sqlite>,
    strand: &Row,
    turns: usize,
    jobs: &keel::Pack,
) -> Result<Retirement, Error> {
    let messages = tx
        .ask(
            &form("Message")
                .when("strand", Op::Eq, &strand.key().to_string())
                .count(),
        )
        .await?
        .count()
        .ok_or_else(|| Error::Adapt("retirement message count missing".into()))?;
    let retirement = Retirement {
        id: read::text(strand, "tag").map_err(Error::Adapt)?.into(),
        ended: santi_model::now(),
        jobs: jobs
            .rows()
            .iter()
            .map(|row| {
                read::text(row, "tag")
                    .map(str::to_string)
                    .map_err(Error::Adapt)
            })
            .collect::<Result<_, _>>()?,
        messages,
        turns,
    };
    let jobs =
        serde_json::to_string(&retirement.jobs).map_err(|error| Error::Adapt(error.to_string()))?;
    let soul = read::int(strand, "soul").map_err(Error::Adapt)?;
    let key = tx
        .put(
            "Retirement",
            &[
                ("tag", &retirement.id),
                ("ended", &retirement.ended),
                ("jobs", &jobs),
                ("messages", &messages.to_string()),
                ("turns", &turns.to_string()),
                ("soul", &soul.to_string()),
            ],
        )
        .await?;
    compacts(tx, strand.key(), key).await?;
    Ok(retirement)
}

async fn compacts(tx: &mut Tx<'_, Sqlite>, strand: i64, retirement: i64) -> Result<(), Error> {
    let compacts = tx
        .ask(&form("Compact").when("strand", Op::Eq, &strand.to_string()))
        .await?;
    let retirement = retirement.to_string();
    for compact in compacts.rows() {
        let tag = read::text(compact, "tag").map_err(Error::Adapt)?;
        tx.put(
            "RetiredCompact",
            &[("tag", tag), ("retirement", &retirement)],
        )
        .await?;
    }
    Ok(())
}

struct Gate<'a, 'tx>(&'a mut Tx<'tx, Sqlite>);

impl Gate<'_, '_> {
    async fn forks(&mut self, strand: &str, tag: &str) -> Result<(), Error> {
        let forks = self
            .0
            .ask(
                &form("Strand")
                    .when("parent", Op::Eq, strand)
                    .order("tag", Rank::Asc),
            )
            .await?;
        if !forks.rows().is_empty() {
            let names = forks
                .rows()
                .iter()
                .filter_map(|row| row.text("tag"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Adapt(format!(
                "strand {tag} is shown by live fork {names}"
            )));
        }
        Ok(())
    }

    async fn inputs(&mut self, strand: &str) -> Result<(), Error> {
        if let Some(row) = self
            .0
            .one(&form("StrandInbox").when("strand", Op::Eq, strand))
            .await?
        {
            return Err(blocked("pending input", &row));
        }
        if let Some(row) = self
            .0
            .one(
                &form("InboxReceipt")
                    .when("strand", Op::Eq, strand)
                    .any("state", &["accepted", "driving", "recovered"]),
            )
            .await?
        {
            return Err(blocked("unfinished receipt", &row));
        }
        Ok(())
    }

    async fn acknowledgements(&mut self, strand: &str) -> Result<(), Error> {
        let receipts = self
            .0
            .ask(&form("InboxReceipt").when("strand", Op::Eq, strand))
            .await?;
        for receipt in receipts.rows() {
            let row = self
                .0
                .one(
                    &form("Acknowledgement")
                        .when("receipt", Op::Eq, &receipt.key().to_string())
                        .any("state", &["dispatching", "unknown"]),
                )
                .await?;
            if let Some(row) = row {
                return Err(blocked("unsettled acknowledgement", &row));
            }
        }
        Ok(())
    }

    async fn turns(&mut self, strand: &str) -> Result<usize, Error> {
        let turns = self
            .0
            .ask(&form("Turn").when("strand", Op::Eq, strand))
            .await?;
        for rows in turns.rows().chunks(128) {
            self.closed(rows).await?;
        }
        Ok(turns.rows().len())
    }

    async fn closed(&mut self, turns: &[Row]) -> Result<(), Error> {
        let keys = turns
            .iter()
            .map(|row| row.key().to_string())
            .collect::<Vec<_>>();
        let keys = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let mut finished = BTreeSet::new();
        for unit in ["TurnCompletion", "TurnFailure"] {
            let rows = self.0.ask(&form(unit).any("turn", &keys)).await?;
            finished.extend(rows.rows().iter().filter_map(|row| row.int("turn")));
        }
        for row in turns {
            if !finished.contains(&row.key()) {
                return Err(blocked("running turn", row));
            }
        }
        let effects = self.0.ask(&form("StrandEffect").any("turn", &keys)).await?;
        for row in effects.rows() {
            if !matches!(
                row.text("state"),
                Some("settled_applied" | "settled_not_applied")
            ) {
                return Err(blocked("unsettled effect", row));
            }
        }
        Ok(())
    }

    async fn jobs(&mut self, strand: &str) -> Result<keel::Pack, Error> {
        let jobs = self
            .0
            .ask(
                &form("Job")
                    .when("strand", Op::Eq, strand)
                    .order("tag", Rank::Asc),
            )
            .await?;
        for row in jobs.rows() {
            if !matches!(
                row.text("state"),
                Some("succeeded" | "failed" | "timed_out" | "cancelled" | "unknown")
            ) {
                return Err(blocked("active job", row));
            }
            if row.text("acknowledged").is_none() {
                return Err(blocked("unacknowledged job", row));
            }
        }
        Ok(jobs)
    }
}

fn blocked(kind: &str, row: &Row) -> Error {
    Error::Adapt(format!(
        "strand retirement conflicts with {kind} {}",
        row.text("tag").unwrap_or("unknown")
    ))
}
