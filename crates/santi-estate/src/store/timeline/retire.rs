use super::{Store, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Rank, Row, Tx, form};
use std::collections::BTreeSet;

impl Store {
    pub async fn end_strand(&self, tag: &str) -> Result<Vec<String>, String> {
        self.core
            .batch(async |tx| {
                let key = read::need(tx, "Strand", "tag", tag).await?;
                let strand = key.to_string();
                Gate(tx).forks(&strand, tag).await?;
                Gate(tx).inputs(&strand).await?;
                Gate(tx).turns(&strand).await?;
                let jobs = Gate(tx).jobs(&strand).await?;
                tx.end("Strand", key).await?;
                Ok(jobs)
            })
            .await
            .map_err(read::error)
    }
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

    async fn turns(&mut self, strand: &str) -> Result<(), Error> {
        let turns = self
            .0
            .ask(&form("Turn").when("strand", Op::Eq, strand))
            .await?;
        for rows in turns.rows().chunks(128) {
            self.closed(rows).await?;
        }
        Ok(())
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

    async fn jobs(&mut self, strand: &str) -> Result<Vec<String>, Error> {
        let jobs = self
            .0
            .ask(&form("Job").when("strand", Op::Eq, strand))
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
        Ok(jobs
            .rows()
            .iter()
            .flat_map(|row| [row.text("tag"), row.text("generation")])
            .flatten()
            .map(str::to_string)
            .collect())
    }
}

fn blocked(kind: &str, row: &Row) -> Error {
    Error::Adapt(format!(
        "strand retirement conflicts with {kind} {}",
        row.text("tag").unwrap_or("unknown")
    ))
}
