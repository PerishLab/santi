use crate::store::{entry, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Row, Tx, form};

pub(in crate::store) struct Reader<'a, 'tx>(pub(in crate::store) &'a mut Tx<'tx, Sqlite>);

impl Reader<'_, '_> {
    pub(in crate::store) async fn ends(&mut self, compact: &Row) -> Result<(Row, Row), Error> {
        let strand = number(compact, "strand")?;
        match (compact.int("head"), compact.int("tail")) {
            (Some(head), Some(tail)) => {
                if compact.int("first").is_some() || compact.int("last").is_some() {
                    return Err(Error::Adapt(
                        "compact has mixed entry and Message boundaries".into(),
                    ));
                }
                let first = self.need(head).await?;
                let last = self.need(tail).await?;
                if first.int("strand") != Some(strand) || last.int("strand") != Some(strand) {
                    return Err(Error::Adapt(
                        "compact boundary belongs to another strand".into(),
                    ));
                }
                Ok((first, last))
            }
            (None, None) => Ok((
                self.legacy(strand, number(compact, "first")?).await?,
                self.legacy(strand, number(compact, "last")?).await?,
            )),
            _ => Err(Error::Adapt(
                "compact entry boundaries must be paired".into(),
            )),
        }
    }

    pub(in crate::store) async fn tag(&mut self, seat: &Row) -> Result<String, Error> {
        let target = entry::aim(self.0, seat).await?;
        read::text(&target, "tag")
            .map(str::to_string)
            .map_err(Error::Adapt)
    }

    async fn need(&mut self, key: i64) -> Result<Row, Error> {
        self.0
            .one(&form("StrandEntry").when("id", Op::Eq, &key.to_string()))
            .await?
            .ok_or_else(|| Error::Missing("compact boundary entry".into()))
    }

    async fn legacy(&mut self, strand: i64, message: i64) -> Result<Row, Error> {
        self.0
            .one(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.to_string())
                    .when("message", Op::Eq, &message.to_string()),
            )
            .await?
            .ok_or_else(|| Error::Missing("compact boundary message entry".into()))
    }
}

pub(in crate::store) fn number(row: &Row, field: &str) -> Result<i64, Error> {
    row.int(field)
        .ok_or_else(|| Error::Adapt(format!("compact {field} missing")))
}
