use super::*;
use crate::inference::{Declaration, Effective, Origin, Patch, Reading};
use crate::sources::{Amendment, Definition, Record};
use santi_estate::Target;

impl Service {
    pub async fn source(
        &self,
        kind: crate::sources::Kind,
        id: i64,
    ) -> Result<Option<Record>, String> {
        self.store.source(kind, id).await
    }

    pub async fn sources(&self) -> Result<Vec<Record>, String> {
        self.store.sources().await
    }

    pub async fn define(&self, definition: Definition) -> Result<Record, String> {
        self.store.register(definition).await
    }

    pub async fn amend(&self, edit: Amendment) -> Result<Record, String> {
        self.store
            .amending(
                edit.expected,
                edit.replacement,
                self.declaration.scenario.as_deref(),
            )
            .await
    }

    pub async fn withdraw(&self, record: Record) -> Result<(), String> {
        self.store
            .withdrawing(record, self.declaration.scenario.as_deref())
            .await
    }

    pub(crate) fn infer(
        &self,
        rows: &[Record],
        target: Target,
        candidate: &Declaration,
        parent: Option<&Declaration>,
    ) -> Result<Effective, String> {
        let layers = match target {
            Target::Soul => vec![
                (&self.declaration, Origin::Service),
                (candidate, Origin::Soul),
            ],
            Target::Strand => vec![
                (&self.declaration, Origin::Service),
                (
                    parent.ok_or_else(|| "soul inference not found".to_string())?,
                    Origin::Soul,
                ),
                (candidate, Origin::Strand),
            ],
        };
        self.providers.selecting(&layers, rows)
    }

    fn reading(
        &self,
        rows: &[Record],
        target: Target,
        candidate: &Declaration,
        parent: Option<&Declaration>,
    ) -> Reading {
        match self.infer(rows, target, candidate, parent) {
            Ok(effective) => Reading {
                declaration: candidate.clone(),
                effective: Some(effective),
                error: None,
            },
            Err(error) => Reading {
                declaration: candidate.clone(),
                effective: None,
                error: Some(error),
            },
        }
    }

    pub async fn inference(&self, target: Target, tag: &str) -> Result<Option<Reading>, String> {
        let rows = self.store.sources().await?;
        match target {
            Target::Soul => {
                let Some(soul) = self.store.soul(tag).await? else {
                    return Ok(None);
                };
                Ok(Some(self.reading(
                    &rows,
                    Target::Soul,
                    &soul.inference,
                    None,
                )))
            }
            Target::Strand => {
                let Some(strand) = self.store.strand(tag).await? else {
                    return Ok(None);
                };
                let soul = self
                    .store
                    .soul(&strand.soul)
                    .await?
                    .ok_or_else(|| "soul not found".to_string())?;
                Ok(Some(self.reading(
                    &rows,
                    Target::Strand,
                    &strand.inference,
                    Some(&soul.inference),
                )))
            }
        }
    }

    pub async fn declare(
        &self,
        target: Target,
        tag: &str,
        patch: &Patch,
    ) -> Result<Reading, String> {
        let created = now();
        let (declaration, effective) = self
            .store
            .change(
                santi_estate::Edit {
                    target,
                    tag,
                    patch,
                    now: &created,
                },
                |target, candidate, parent, rows| {
                    self.infer(rows, target, candidate, parent)
                        .map_err(|error| format!("inference rejected: {error}"))
                },
            )
            .await?;
        Ok(Reading {
            declaration,
            effective: Some(effective),
            error: None,
        })
    }

    pub async fn weaving(
        &self,
        inference: Option<&Declaration>,
    ) -> Result<strand::Created, String> {
        self.seating(crate::GENESIS, inference).await
    }

    pub async fn seating(
        &self,
        soul: &str,
        inference: Option<&Declaration>,
    ) -> Result<strand::Created, String> {
        let soul = soul.trim();
        if soul.is_empty() {
            return Err("soul must not be empty".to_string());
        }
        if self.store.soul(soul).await?.is_none() {
            return Err("soul not found".to_string());
        }
        let created = now();
        let (strand, _) = self
            .store
            .create(
                santi_estate::Strand {
                    tag: &tag("ss"),
                    soul,
                    label: None,
                    parent: None,
                    fork: None,
                    inference,
                    created: &created,
                },
                |target, candidate, parent, rows| {
                    self.infer(rows, target, candidate, parent)
                        .map_err(|error| format!("inference rejected: {error}"))
                },
            )
            .await?;
        Ok(strand::Created { strand })
    }
}
