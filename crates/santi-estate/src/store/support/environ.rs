use super::super::Store;
use super::read;
use keel::{Op, Rank, form};
use santi_model::environ;

#[derive(Clone, Copy)]
pub struct EnvironDraft<'a> {
    pub scope: environ::Scope,
    pub owner: &'a str,
    pub name: &'a str,
    pub value: &'a str,
    pub occurred: &'a str,
}

struct Seat {
    unit: &'static str,
    field: &'static str,
    owner: String,
}

impl Store {
    pub async fn set_environ(&self, draft: EnvironDraft<'_>) -> Result<environ::Variable, String> {
        let seat = self.seat(draft.scope, draft.owner).await?;
        self.core
            .batch(async |tx| {
                let held = tx
                    .one(&form(seat.unit).when(seat.field, Op::Eq, &seat.owner).when(
                        "name",
                        Op::Eq,
                        draft.name,
                    ))
                    .await?;
                match held {
                    Some(row) if row.text("value") == Some(draft.value) => {}
                    Some(row) => {
                        tx.set(
                            seat.unit,
                            row.key(),
                            &[("value", draft.value), ("updated", draft.occurred)],
                        )
                        .await?;
                    }
                    None => {
                        tx.put(
                            seat.unit,
                            &[
                                ("name", draft.name),
                                ("value", draft.value),
                                ("created", draft.occurred),
                                ("updated", draft.occurred),
                                (seat.field, seat.owner.as_str()),
                            ],
                        )
                        .await?;
                    }
                }
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.environ(draft.scope, draft.owner, draft.name)
            .await?
            .ok_or_else(|| "written environment variable missing".to_string())
    }

    pub async fn environ(
        &self,
        scope: environ::Scope,
        owner: &str,
        name: &str,
    ) -> Result<Option<environ::Variable>, String> {
        let Ok(seat) = self.seat(scope, owner).await else {
            return Ok(None);
        };
        let row = self
            .core
            .one(
                &form(seat.unit)
                    .when(seat.field, Op::Eq, &seat.owner)
                    .when("name", Op::Eq, name),
            )
            .await
            .map_err(read::error)?;
        row.as_ref()
            .map(|row| variable(row, scope, owner))
            .transpose()
    }

    pub async fn environs(
        &self,
        scope: environ::Scope,
        owner: &str,
    ) -> Result<Vec<environ::Variable>, String> {
        let seat = self.seat(scope, owner).await?;
        let rows = self
            .core
            .ask(
                &form(seat.unit)
                    .when(seat.field, Op::Eq, &seat.owner)
                    .order("name", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        rows.rows()
            .iter()
            .map(|row| variable(row, scope, owner))
            .collect()
    }

    pub async fn end_environ(
        &self,
        scope: environ::Scope,
        owner: &str,
        name: &str,
    ) -> Result<bool, String> {
        let seat = self.seat(scope, owner).await?;
        self.core
            .batch(async |tx| {
                let Some(row) = tx
                    .one(&form(seat.unit).when(seat.field, Op::Eq, &seat.owner).when(
                        "name",
                        Op::Eq,
                        name,
                    ))
                    .await?
                else {
                    return Ok(false);
                };
                tx.end(seat.unit, row.key()).await?;
                Ok(true)
            })
            .await
            .map_err(read::error)
    }

    async fn seat(&self, scope: environ::Scope, owner: &str) -> Result<Seat, String> {
        let (unit, field) = match scope {
            environ::Scope::Soul => ("SoulEnviron", "soul"),
            environ::Scope::Strand => ("StrandEnviron", "strand"),
        };
        let row = read::one(&self.core, scope.unit(), "tag", owner)
            .await?
            .ok_or_else(|| format!("{} not found", scope.encode()))?;
        Ok(Seat {
            unit,
            field,
            owner: row.key().to_string(),
        })
    }
}

fn variable(
    row: &keel::Row,
    scope: environ::Scope,
    owner: &str,
) -> Result<environ::Variable, String> {
    Ok(environ::Variable {
        scope,
        owner: owner.to_string(),
        name: read::text(row, "name")?.to_string(),
        value: read::text(row, "value")?.to_string(),
        created: read::text(row, "created")?.to_string(),
        updated: read::text(row, "updated")?.to_string(),
    })
}
