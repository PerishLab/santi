use super::{Store, read};

use keel::adapt::db::Sqlite;
use keel::{Op, Rank, Row, Tx, form};

mod checkpoint;
mod copy;

#[derive(Clone, Copy)]
pub struct ForkDraft<'a> {
    pub tag: &'a str,
    pub parent: &'a str,
    pub at: i64,
    pub inference: Option<&'a santi_model::inference::Declaration>,
    pub created: &'a str,
}

struct Writer<'a, 'tx>(&'a mut Tx<'tx, Sqlite>);

impl Store {
    pub async fn branch(
        &self,
        draft: ForkDraft<'_>,
    ) -> Result<santi_model::strand::Strand, String> {
        self.branching(draft, |_, _, _| Ok(()))
            .await
            .map(|(strand, _)| strand)
    }

    pub async fn branching<V>(
        &self,
        draft: ForkDraft<'_>,
        validate: impl Fn(
            crate::Target,
            &santi_model::inference::Declaration,
            Option<&santi_model::inference::Declaration>,
        ) -> Result<V, String>,
    ) -> Result<(santi_model::strand::Strand, Option<V>), String> {
        let tag = draft.tag.to_string();
        let checked = self
            .core
            .batch(async |tx| {
                let parent = Writer(tx).need("Strand", draft.parent).await?;
                let at = checkpoint::selected(tx, &parent, draft.at).await?;
                creating(tx, ForkDraft { at, ..draft }, &validate).await
            })
            .await
            .map_err(read::error)?;
        let strand = self
            .strand(&tag)
            .await?
            .ok_or_else(|| "forked strand missing".to_string())?;
        Ok((strand, checked))
    }

    pub async fn fork(&self, draft: ForkDraft<'_>) -> Result<santi_model::strand::Strand, String> {
        self.forking(draft, |_, _, _| Ok(()))
            .await
            .map(|(strand, _)| strand)
    }

    pub async fn forking<V>(
        &self,
        draft: ForkDraft<'_>,
        validate: impl Fn(
            crate::Target,
            &santi_model::inference::Declaration,
            Option<&santi_model::inference::Declaration>,
        ) -> Result<V, String>,
    ) -> Result<(santi_model::strand::Strand, Option<V>), String> {
        if draft.at < 0 {
            return Err("fork must be >= 0".to_string());
        }
        let tag = draft.tag.to_string();
        let checked = self
            .core
            .batch(async |tx| creating(tx, draft, &validate).await)
            .await
            .map_err(read::error)?;
        let strand = self
            .strand(&tag)
            .await?
            .ok_or_else(|| "forked strand missing".to_string())?;
        Ok((strand, checked))
    }

    pub async fn discard_fork(&self, tag: &str) -> Result<bool, String> {
        self.core
            .batch(async |tx| Writer(tx).discard(tag).await)
            .await
            .map_err(read::error)
    }
}

async fn creating<V>(
    tx: &mut Tx<'_, Sqlite>,
    draft: ForkDraft<'_>,
    validate: &impl Fn(
        crate::Target,
        &santi_model::inference::Declaration,
        Option<&santi_model::inference::Declaration>,
    ) -> Result<V, String>,
) -> Result<Option<V>, keel::adapt::Error> {
    let parent = Writer(tx).need("Strand", draft.parent).await?;
    let inherited = int(&parent, "next")?.saturating_sub(1);
    if draft.at > inherited {
        return Err(keel::adapt::Error::Adapt(format!(
            "fork {} is past parent end {inherited}",
            draft.at
        )));
    }
    let soul = int(&parent, "soul")?.to_string();
    let checked = if let Some(inference) = draft.inference {
        inference.validate().map_err(keel::adapt::Error::Adapt)?;
        let row = tx
            .one(&form("Soul").when("id", Op::Eq, &soul))
            .await?
            .ok_or_else(|| keel::adapt::Error::Adapt(format!("Soul {soul} not found")))?;
        let lower = crate::store::support::inference::declaration(&row)
            .map_err(keel::adapt::Error::Adapt)?;
        Some(
            validate(crate::Target::Strand, inference, Some(&lower))
                .map_err(keel::adapt::Error::Adapt)?,
        )
    } else {
        None
    };
    let (provider, model, effort) = match draft.inference {
        Some(inference) => (
            inference.provider.clone(),
            inference.model.clone(),
            inference.reasoning_effort.clone(),
        ),
        None => (
            parent.text("provider").map(str::to_string),
            parent.text("model").map(str::to_string),
            parent.text("reasoning_effort").map(str::to_string),
        ),
    };
    let next = (draft.at + 1).to_string();
    let seen = int(&parent, "seen")?.min(draft.at).to_string();
    let fork = draft.at.to_string();
    let parent_key = parent.key().to_string();
    let mut fields = vec![
        ("tag", draft.tag),
        ("next", next.as_str()),
        ("seen", seen.as_str()),
        ("fork", fork.as_str()),
        ("created", draft.created),
        ("updated", draft.created),
        ("soul", soul.as_str()),
        ("parent", parent_key.as_str()),
    ];
    if let Some(provider) = provider.as_deref() {
        fields.push(("provider", provider));
    }
    if let Some(model) = model.as_deref() {
        fields.push(("model", model));
    }
    if let Some(effort) = effort.as_deref() {
        fields.push(("reasoning_effort", effort));
    }
    let child = tx.put("Strand", &fields).await?;
    copy::entries(tx, parent.key(), child, draft.at).await?;
    copy::compacts(tx, parent.key(), child, draft.at).await?;
    super::material::binding::inherit(tx, parent.key(), child, draft.created).await?;
    Ok(checked)
}

impl Writer<'_, '_> {
    async fn discard(&mut self, tag: &str) -> Result<bool, keel::adapt::Error> {
        let Some(child) = self.0.one(&form("Strand").when("tag", Op::Eq, tag)).await? else {
            return Ok(false);
        };
        if child.int("parent").is_none() {
            return Err(keel::adapt::Error::Adapt(
                "refusing to discard a non-fork strand".into(),
            ));
        }
        let fork = int(&child, "fork")?;
        if int(&child, "next")? != fork + 1 {
            return Err(keel::adapt::Error::Adapt(
                "refusing to discard a fork with new timeline entries".into(),
            ));
        }
        for unit in [
            "Turn",
            "StrandInbox",
            "InboxSlot",
            "InboxReceipt",
            "Job",
            "JobCapability",
        ] {
            if self
                .0
                .one(&form(unit).when("strand", Op::Eq, &child.key().to_string()))
                .await?
                .is_some()
            {
                return Err(keel::adapt::Error::Adapt(format!(
                    "refusing to discard a fork with live {unit}"
                )));
            }
        }
        self.0.end("Strand", child.key()).await?;
        Ok(true)
    }

    async fn message_sequence(
        &mut self,
        strand: i64,
        message: i64,
    ) -> Result<i64, keel::adapt::Error> {
        self.0
            .one(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.to_string())
                    .when("message", Op::Eq, &message.to_string()),
            )
            .await?
            .and_then(|row| row.int("sequence"))
            .ok_or_else(|| keel::adapt::Error::Missing("compact boundary entry".into()))
    }

    async fn need(&mut self, unit: &str, tag: &str) -> Result<Row, keel::adapt::Error> {
        self.0
            .one(&form(unit).when("tag", Op::Eq, tag))
            .await?
            .ok_or_else(|| keel::adapt::Error::Missing(format!("{unit} {tag}")))
    }
}

fn text<'a>(row: &'a Row, field: &str) -> Result<&'a str, keel::adapt::Error> {
    row.text(field)
        .ok_or_else(|| keel::adapt::Error::Adapt(format!("fork {field} missing")))
}

fn int(row: &Row, field: &str) -> Result<i64, keel::adapt::Error> {
    row.int(field)
        .ok_or_else(|| keel::adapt::Error::Adapt(format!("fork {field} missing")))
}
