use super::super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Soul,
    Strand,
}

impl Target {
    fn unit(self) -> &'static str {
        match self {
            Target::Soul => "Soul",
            Target::Strand => "Strand",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Edit<'a> {
    pub target: Target,
    pub tag: &'a str,
    pub patch: &'a santi_model::inference::Patch,
    pub now: &'a str,
}

impl Store {
    pub async fn declare(
        &self,
        target: Target,
        tag: &str,
        patch: &santi_model::inference::Patch,
        now: &str,
    ) -> Result<santi_model::inference::Declaration, String> {
        self.change(
            Edit {
                target,
                tag,
                patch,
                now,
            },
            |_, _, _| Ok(()),
        )
        .await
        .map(|(declaration, ())| declaration)
    }

    pub async fn change<V>(
        &self,
        edit: Edit<'_>,
        validate: impl Fn(
            Target,
            &santi_model::inference::Declaration,
            Option<&santi_model::inference::Declaration>,
        ) -> Result<V, String>,
    ) -> Result<(santi_model::inference::Declaration, V), String> {
        let unit = edit.target.unit();
        self.core
            .batch(async |tx| {
                let row = tx
                    .one(&keel::form(unit).when("tag", keel::Op::Eq, edit.tag))
                    .await?
                    .ok_or(keel::adapt::Error::Adapt(format!(
                        "{unit} {} not found",
                        edit.tag
                    )))?;
                let current = declaration(&row).map_err(keel::adapt::Error::Adapt)?;
                let parent = if edit.target == Target::Soul {
                    None
                } else {
                    let soul = row.int("soul").ok_or(keel::adapt::Error::Adapt(format!(
                        "Strand {} has no soul",
                        edit.tag
                    )))?;
                    let row = tx
                        .one(&keel::form("Soul").when("id", keel::Op::Eq, &soul.to_string()))
                        .await?
                        .ok_or(keel::adapt::Error::Adapt(format!("Soul {soul} not found")))?;
                    Some(declaration(&row).map_err(keel::adapt::Error::Adapt)?)
                };
                let next = edit.patch.apply(&current);
                next.validate().map_err(keel::adapt::Error::Adapt)?;
                let verified = validate(edit.target, &next, parent.as_ref())
                    .map_err(keel::adapt::Error::Adapt)?;
                tx.set(unit, row.key(), &fields(&next, edit.now)).await?;
                let cleared = cleared(&next);
                if !cleared.is_empty() {
                    tx.unset(unit, row.key(), &cleared).await?;
                }
                Ok((next, verified))
            })
            .await
            .map_err(read::error)
    }
}

pub(in crate::store) fn declaration(
    row: &keel::Row,
) -> Result<santi_model::inference::Declaration, String> {
    Ok(santi_model::inference::Declaration {
        provider: row.text("provider").map(str::to_string),
        model: row.text("model").map(str::to_string),
        reasoning_effort: row.text("reasoning_effort").map(str::to_string),
    })
}

fn fields<'a>(
    inference: &'a santi_model::inference::Declaration,
    now: &'a str,
) -> Vec<(&'a str, &'a str)> {
    let mut fields = vec![("updated", now)];
    if let Some(provider) = inference.provider.as_deref() {
        fields.push(("provider", provider));
    }
    if let Some(model) = inference.model.as_deref() {
        fields.push(("model", model));
    }
    if let Some(effort) = inference.reasoning_effort.as_deref() {
        fields.push(("reasoning_effort", effort));
    }
    fields
}

fn cleared(inference: &santi_model::inference::Declaration) -> Vec<&'static str> {
    let mut fields = Vec::new();
    if inference.provider.is_none() {
        fields.push("provider");
    }
    if inference.model.is_none() {
        fields.push("model");
    }
    if inference.reasoning_effort.is_none() {
        fields.push("reasoning_effort");
    }
    fields
}

pub struct StrandDraft<'a> {
    pub tag: &'a str,
    pub soul: &'a str,
    pub label: Option<&'a str>,
    pub parent: Option<&'a str>,
    pub fork: Option<i64>,
    pub inference: Option<&'a santi_model::inference::Declaration>,
    pub created: &'a str,
}

impl Store {
    pub async fn create_strand(
        &self,
        draft: StrandDraft<'_>,
    ) -> Result<santi_model::strand::Strand, String> {
        self.create(draft, |_, _, _| Ok(()))
            .await
            .map(|(strand, _)| strand)
    }

    pub async fn create<V>(
        &self,
        draft: StrandDraft<'_>,
        validate: impl Fn(
            Target,
            &santi_model::inference::Declaration,
            Option<&santi_model::inference::Declaration>,
        ) -> Result<V, String>,
    ) -> Result<(santi_model::strand::Strand, Option<V>), String> {
        let tag = draft.tag.to_string();
        let verified = self
            .core
            .batch(async |tx| {
                let soul = tx
                    .one(&keel::form("Soul").when("tag", keel::Op::Eq, draft.soul))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(format!("Soul {}", draft.soul)))?;
                let parent = match draft.parent {
                    Some(parent) => Some(read::need(tx, "Strand", "tag", parent).await?),
                    None => None,
                };
                let soul_key = soul.key().to_string();
                let parent_key = parent.map(|key| key.to_string());
                let fork = draft.fork.map(|at| at.to_string());
                let mut fields = vec![
                    ("tag", draft.tag),
                    ("created", draft.created),
                    ("updated", draft.created),
                    ("soul", soul_key.as_str()),
                ];
                if let Some(label) = draft.label {
                    fields.push(("label", label));
                }
                if let Some(parent) = parent_key.as_deref() {
                    fields.push(("parent", parent));
                }
                if let Some(fork) = fork.as_deref() {
                    fields.push(("fork", fork));
                }
                let mut checked = None;
                if let Some(inference) = draft.inference {
                    inference.validate().map_err(keel::adapt::Error::Adapt)?;
                    let lower = declaration(&soul).map_err(keel::adapt::Error::Adapt)?;
                    checked = Some(
                        validate(Target::Strand, inference, Some(&lower))
                            .map_err(keel::adapt::Error::Adapt)?,
                    );
                    if let Some(provider) = inference.provider.as_deref() {
                        fields.push(("provider", provider));
                    }
                    if let Some(model) = inference.model.as_deref() {
                        fields.push(("model", model));
                    }
                    if let Some(effort) = inference.reasoning_effort.as_deref() {
                        fields.push(("reasoning_effort", effort));
                    }
                }
                tx.put("Strand", &fields).await?;
                Ok(checked)
            })
            .await
            .map_err(read::error)?;
        let strand = self
            .strand(&tag)
            .await?
            .ok_or_else(|| "created strand missing".to_string())?;
        Ok((strand, verified))
    }
}
