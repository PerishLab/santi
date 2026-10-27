use super::{Reader, Seat, Store, adapt, codec, read};
use keel::{Op, Rank, Row, Tree, form};
use santi_error::{History, Incident, Scope};

impl Store {
    pub async fn history(
        &self,
        scope: &Scope,
        limit: usize,
        before: Option<&str>,
    ) -> Result<Option<Vec<Incident>>, String> {
        self.core
            .batch(async |tx| {
                let mut reader = Reader(tx);
                let Some(seat) = reader.seat(scope).await? else {
                    return Ok(None);
                };
                let rows = reader.page(&seat, limit.clamp(1, 1000), before).await?;
                rows.iter()
                    .map(|row| codec::incident(row, scope).map_err(adapt))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some)
            })
            .await
            .map_err(read::error)
    }

    pub async fn inspect(&self, tag: &str) -> Result<Option<History>, String> {
        self.core
            .batch(async |tx| {
                let reader = Reader(tx);
                for (unit, owner, field) in [
                    ("SoulIncident", "Soul", "soul"),
                    ("StrandIncident", "Strand", "strand"),
                ] {
                    let Some(row) = reader.0.one(&form(unit).when("tag", Op::Eq, tag)).await?
                    else {
                        continue;
                    };
                    let key = read::int(&row, field).map_err(adapt)?.to_string();
                    let owner = reader
                        .0
                        .one(&form(owner).when("id", Op::Eq, &key))
                        .await?
                        .ok_or_else(|| adapt("incident owner missing".into()))?;
                    let scope = Scope::new(field, read::text(&owner, "tag").map_err(adapt)?);
                    return reader.inspect(&row, &scope).map(Some);
                }
                Ok(None)
            })
            .await
            .map_err(read::error)
    }
}

impl Reader<'_, '_> {
    async fn page(
        &mut self,
        seat: &Seat,
        limit: usize,
        before: Option<&str>,
    ) -> Result<Vec<Row>, keel::adapt::Error> {
        let query = scoped(seat);
        let Some(before) = before else {
            return Ok(self.0.ask(&query.top(limit)).await?.rows().to_vec());
        };
        let cursor = self
            .0
            .one(&scoped(seat).when("tag", Op::Eq, before))
            .await?
            .ok_or_else(|| adapt("incident cursor is not retained in this scope".into()))?;
        let time = read::text(&cursor, "first_seen").map_err(adapt)?;
        let mut rows = self
            .0
            .ask(
                &query
                    .clone()
                    .when("first_seen", Op::Eq, time)
                    .when("tag", Op::Lt, before)
                    .top(limit),
            )
            .await?
            .rows()
            .to_vec();
        if rows.len() < limit {
            rows.extend(
                self.0
                    .ask(
                        &query
                            .when("first_seen", Op::Lt, time)
                            .top(limit - rows.len()),
                    )
                    .await?
                    .rows()
                    .iter()
                    .cloned(),
            );
        }
        Ok(rows)
    }

    fn inspect(&self, row: &Row, scope: &Scope) -> Result<History, keel::adapt::Error> {
        let incident = codec::incident(row, scope).map_err(adapt)?;
        let transitions = ["opened_payload", "resolved_payload"]
            .into_iter()
            .filter_map(|field| row.text(field))
            .map(codec::transition)
            .collect::<Result<Vec<_>, _>>()
            .map_err(adapt)?;
        Ok(History {
            incident,
            transitions,
        })
    }
}

fn scoped(seat: &Seat) -> Tree {
    form(seat.unit)
        .when(seat.field, Op::Eq, &seat.owner.to_string())
        .order("first_seen", Rank::Desc)
        .order("tag", Rank::Desc)
}
