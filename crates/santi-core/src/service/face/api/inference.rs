use super::*;
use crate::inference::{Declaration, Effective, Origin, Patch, Reading};
use crate::sources::{Amendment, Definition, Record};
use santi_estate::Target;

impl Service {
    pub async fn sources(&self) -> Result<Vec<Record>, String> {
        self.store.sources().await
    }

    pub async fn define(&self, definition: Definition) -> Result<Record, String> {
        self.store.register(definition).await
    }

    fn referenced(&self, record: &Record) -> bool {
        matches!(&record.definition, Definition::Access { name, .. } | Definition::Alias { name, .. }
            if self.declaration.provider.as_ref() == Some(name))
    }

    pub async fn amend(&self, edit: Amendment) -> Result<Record, String> {
        if self.referenced(&edit.expected)
            && edit.expected.definition.name() != edit.replacement.name()
        {
            return Err("source referenced by service declaration".into());
        }
        self.store.amend(edit.expected, edit.replacement).await
    }

    pub async fn withdraw(&self, record: Record) -> Result<(), String> {
        if self.referenced(&record) {
            return Err("source referenced by service declaration".into());
        }
        self.store.withdraw(record).await
    }

    pub(crate) fn infer(
        &self,
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
        self.providers.resolve(&layers)
    }

    fn reading(
        &self,
        target: Target,
        candidate: &Declaration,
        parent: Option<&Declaration>,
    ) -> Reading {
        match self.infer(target, candidate, parent) {
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
        match target {
            Target::Soul => {
                let Some(soul) = self.store.soul(tag).await? else {
                    return Ok(None);
                };
                Ok(Some(self.reading(Target::Soul, &soul.inference, None)))
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
                |target, candidate, parent| {
                    self.infer(target, candidate, parent)
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
                santi_estate::StrandDraft {
                    tag: &tag("ss"),
                    soul,
                    label: None,
                    parent: None,
                    fork: None,
                    inference,
                    created: &created,
                },
                |target, candidate, parent| {
                    self.infer(target, candidate, parent)
                        .map_err(|error| format!("inference rejected: {error}"))
                },
            )
            .await?;
        Ok(strand::Created { strand })
    }
}
