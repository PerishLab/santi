use super::{Store, read};
use keel::adapt::db::Sqlite;
use keel::{Op, Rank, Tx, form};
use santi_error::{Fault, Incident, Scope, Transition};

mod codec;
mod persist;

const UNITS: [&str; 2] = ["SoulIncident", "StrandIncident"];
const EVENTS: [(&str, &str, &str); 4] = [
    ("SoulIncident", "opened_event", "opened_delivered"),
    ("SoulIncident", "resolved_event", "resolved_delivered"),
    ("StrandIncident", "opened_event", "opened_delivered"),
    ("StrandIncident", "resolved_event", "resolved_delivered"),
];

pub(in crate::store) struct Resolution<'a> {
    pub key: &'a str,
    pub by: &'a str,
    pub context: serde_json::Value,
    pub now: &'a str,
}

pub(super) struct Seat {
    unit: &'static str,
    field: &'static str,
    owner: i64,
    scope: Scope,
}

impl Store {
    pub async fn raise(&self, draft: santi_error::Draft, now: &str) -> Result<Fault, String> {
        self.core
            .batch(async |tx| raise_in(tx, draft, now).await)
            .await
            .map_err(read::error)
    }

    pub async fn resolve(
        &self,
        key: &str,
        by: &str,
        context: serde_json::Value,
        now: &str,
    ) -> Result<bool, String> {
        self.core
            .batch(async |tx| {
                resolve_in(
                    tx,
                    Resolution {
                        key,
                        by,
                        context,
                        now,
                    },
                )
                .await
            })
            .await
            .map_err(read::error)
    }

    pub async fn incident(&self, key: &str) -> Result<Option<Incident>, String> {
        self.core
            .batch(async |tx| {
                let Some((code, scope)) = split(key) else {
                    return Ok(None);
                };
                let Some(seat) = Reader(tx).seat(&scope).await? else {
                    return Ok(None);
                };
                let row = Reader(tx).active(&seat, code).await?;
                row.map(|row| codec::incident(&row, &seat.scope).map_err(adapt))
                    .transpose()
            })
            .await
            .map_err(read::error)
    }

    pub async fn incidents(&self, scope: &Scope, limit: usize) -> Result<Vec<Incident>, String> {
        let limit = limit.clamp(1, 1000);
        self.core
            .batch(async |tx| {
                let Some(seat) = Reader(tx).seat(scope).await? else {
                    return Ok(Vec::new());
                };
                let rows = tx
                    .ask(
                        &form(seat.unit)
                            .when(seat.field, Op::Eq, &seat.owner.to_string())
                            .order("first_seen", Rank::Desc)
                            .order("tag", Rank::Desc)
                            .top(limit),
                    )
                    .await?;
                rows.rows()
                    .iter()
                    .map(|row| codec::incident(row, &seat.scope).map_err(adapt))
                    .collect()
            })
            .await
            .map_err(read::error)
    }

    pub async fn active_incident_count(&self, code: &str) -> Result<usize, String> {
        let mut count = 0;
        for unit in UNITS {
            count += self
                .core
                .ask(&form(unit).when("active", Op::Eq, code))
                .await
                .map_err(read::error)?
                .rows()
                .len();
        }
        Ok(count)
    }

    pub async fn pending_errors(&self, limit: usize) -> Result<Vec<Transition>, String> {
        let mut payloads = Vec::new();
        for unit in UNITS {
            let opened = self
                .core
                .ask(&form(unit).missing("opened_delivered"))
                .await
                .map_err(read::error)?;
            for row in opened.rows() {
                payloads.push(read::text(row, "opened_payload")?.to_string());
            }
            let resolved = self
                .core
                .ask(&form(unit).missing("resolved_delivered"))
                .await
                .map_err(read::error)?;
            for row in resolved.rows() {
                if let Some(payload) = row.text("resolved_payload") {
                    payloads.push(payload.to_string());
                }
            }
        }
        let mut transitions = payloads
            .iter()
            .map(|payload| codec::transition(payload))
            .collect::<Result<Vec<_>, _>>()?;
        transitions.sort_by(|left, right| {
            left.occurred
                .cmp(&right.occurred)
                .then_with(|| left.id.cmp(&right.id))
        });
        transitions.truncate(limit);
        Ok(transitions)
    }

    pub async fn deliver_error(&self, transition: &str, delivered: &str) -> Result<(), String> {
        self.core
            .batch(async |tx| {
                for (unit, event, mark) in EVENTS {
                    let Some(row) = tx.one(&form(unit).when(event, Op::Eq, transition)).await?
                    else {
                        continue;
                    };
                    if row.text(mark).is_none() {
                        tx.set(unit, row.key(), &[(mark, delivered)]).await?;
                    }
                    return Ok(());
                }
                Ok(())
            })
            .await
            .map_err(read::error)
    }
}

pub(in crate::store) async fn raise_in(
    tx: &mut Tx<'_, Sqlite>,
    draft: santi_error::Draft,
    now: &str,
) -> Result<Fault, keel::adapt::Error> {
    if draft.key != draft.descriptor.key(&draft.scope.kind, &draft.scope.id) {
        return Err(keel::adapt::Error::Adapt(
            "incident key does not match its descriptor and scope".into(),
        ));
    }
    let seat = Reader(tx).seat(&draft.scope).await?.ok_or_else(|| {
        keel::adapt::Error::Missing(format!("{} {}", draft.scope.kind, draft.scope.id))
    })?;
    let row = Reader(tx).active(&seat, draft.descriptor.code).await?;
    let existing = row
        .as_ref()
        .map(|row| codec::incident(row, &seat.scope).map_err(adapt))
        .transpose()?;
    let mutation = santi_error::engine().open(existing.as_ref(), draft, now);
    persist::Writer(tx)
        .opened(&seat, row.map(|row| row.key()), &mutation)
        .await?;
    Ok(mutation.error)
}

pub(in crate::store) async fn resolve_in(
    tx: &mut Tx<'_, Sqlite>,
    resolution: Resolution<'_>,
) -> Result<bool, keel::adapt::Error> {
    let Some((code, scope)) = split(resolution.key) else {
        return Ok(false);
    };
    let Some(seat) = Reader(tx).seat(&scope).await? else {
        return Ok(false);
    };
    let Some(row) = Reader(tx).active(&seat, code).await? else {
        return Ok(false);
    };
    let active = codec::incident(&row, &seat.scope).map_err(adapt)?;
    let mutation =
        santi_error::engine().resolve(&active, resolution.by, resolution.context, resolution.now);
    persist::Writer(tx)
        .resolved(&seat, row.key(), &mutation)
        .await?;
    Ok(true)
}

pub(in crate::store) async fn incident_key(
    tx: &mut Tx<'_, Sqlite>,
    tag: &str,
) -> Result<i64, keel::adapt::Error> {
    read::need(tx, "StrandIncident", "tag", tag).await
}

struct Reader<'a, 'tx>(&'a mut Tx<'tx, Sqlite>);

impl Reader<'_, '_> {
    async fn seat(&mut self, scope: &Scope) -> Result<Option<Seat>, keel::adapt::Error> {
        let (unit, owner, field) = match scope.kind.as_str() {
            "soul" => ("SoulIncident", "Soul", "soul"),
            "strand" => ("StrandIncident", "Strand", "strand"),
            _ => return Ok(None),
        };
        let Some(row) = self
            .0
            .one(&form(owner).when("tag", Op::Eq, &scope.id))
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(Seat {
            unit,
            field,
            owner: row.key(),
            scope: scope.clone(),
        }))
    }

    async fn active(
        &mut self,
        seat: &Seat,
        code: &str,
    ) -> Result<Option<keel::Row>, keel::adapt::Error> {
        self.0
            .one(
                &form(seat.unit)
                    .when(seat.field, Op::Eq, &seat.owner.to_string())
                    .when("active", Op::Eq, code),
            )
            .await
    }
}

fn split(key: &str) -> Option<(&str, Scope)> {
    let mut parts = key.rsplitn(3, ':');
    let id = parts.next()?;
    let kind = parts.next()?;
    let code = parts.next()?;
    Some((code, Scope::new(kind, id)))
}

fn adapt(error: String) -> keel::adapt::Error {
    keel::adapt::Error::Adapt(error)
}
