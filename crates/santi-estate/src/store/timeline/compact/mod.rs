use crate::store::{Store, read};
use keel::{Op, Rank, form};
use santi_model::compact;

mod admit;
pub(in crate::store) mod boundary;
mod page;
mod plan;
mod rule;
mod scan;
mod scope;
mod selection;
mod thread;

#[derive(Clone, Copy)]
pub struct Limits {
    pub slots: usize,
    pub settled: usize,
}

pub struct Compact<'a> {
    pub tag: &'a str,
    pub strand: &'a str,
    pub first: &'a str,
    pub last: &'a str,
    pub summary: &'a str,
    pub metadata: Option<&'a serde_json::Value>,
    pub expected: Option<&'a compact::Report>,
    pub created: &'a str,
}

impl Store {
    pub async fn preview(
        &self,
        tag: &str,
        strand: &str,
        first: &str,
        last: &str,
    ) -> Result<compact::Report, String> {
        let plan = self
            .core
            .batch(async |tx| plan::build(tx, strand, first, last).await)
            .await
            .map_err(read::error)?;
        Ok(plan.report(tag, true))
    }

    pub async fn condense(&self, draft: Compact<'_>) -> Result<compact::Report, String> {
        if draft.summary.trim().is_empty() {
            return Err("compact summary must not be empty".to_string());
        }
        let metadata = draft
            .metadata
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        let plan = self
            .core
            .batch(async |tx| create(tx, &draft, metadata.as_deref()).await)
            .await
            .map_err(read::error)?;
        Ok(plan)
    }

    pub async fn compact(&self, tag: &str) -> Result<Option<compact::Compact>, String> {
        let Some(row) = read::one(&self.core, "Compact", "tag", tag).await? else {
            return Ok(None);
        };
        decode(self, &row).await.map(Some)
    }

    pub async fn compacts(&self, strand: &str) -> Result<Vec<compact::Compact>, String> {
        let strand = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let rows = self
            .core
            .ask(
                &form("Compact")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .order("created", Rank::Asc)
                    .order("tag", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let mut held = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            held.push(decode(self, row).await?);
        }
        Ok(held)
    }

    pub async fn seated(&self, strand: &str, sequence: i64) -> Result<Option<String>, String> {
        let Some(strand) = read::one(&self.core, "Strand", "tag", strand).await? else {
            return Ok(None);
        };
        let row = self
            .core
            .one(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .when("sequence", Op::Eq, &sequence.to_string())
                    .when("target_type", Op::Eq, "message"),
            )
            .await
            .map_err(read::error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let message = crate::store::entry::target(&self.core, &row).await?;
        Ok(Some(read::text(&message, "tag")?.to_string()))
    }

    pub async fn page(
        &self,
        tag: &str,
        keyword: Option<&str>,
        index: i64,
        size: i64,
    ) -> Result<Option<compact::Page>, String> {
        match self.lookup(tag, keyword, index, size).await? {
            compact::Lookup::Live(page) => Ok(Some(page)),
            compact::Lookup::Retired { .. } | compact::Lookup::Missing => Ok(None),
        }
    }

    pub async fn lookup(
        &self,
        tag: &str,
        keyword: Option<&str>,
        index: i64,
        size: i64,
    ) -> Result<compact::Lookup, String> {
        self.core
            .batch(async |tx| {
                page::Reader(tx)
                    .read(page::Query {
                        tag,
                        keyword,
                        index,
                        size,
                    })
                    .await
            })
            .await
            .map_err(read::error)
    }
}

async fn decode(store: &Store, row: &keel::Row) -> Result<compact::Compact, String> {
    let (first, last) = store
        .core
        .batch(async |tx| {
            let (head, tail) = boundary::Reader(tx).ends(row).await?;
            Ok((
                boundary::Reader(tx).tag(&head).await?,
                boundary::Reader(tx).tag(&tail).await?,
            ))
        })
        .await
        .map_err(read::error)?;
    Ok(compact::Compact {
        id: read::text(row, "tag")?.to_string(),
        strand: read::related(&store.core, "Strand", read::int(row, "strand")?).await?,
        summary: read::text(row, "summary")?.to_string(),
        first,
        last,
        created: row.text("created").map(str::to_string),
        metadata: row
            .text("metadata")
            .map(serde_json::from_str)
            .transpose()
            .map_err(|error| error.to_string())?,
    })
}

pub(in crate::store) async fn create(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    draft: &Compact<'_>,
    metadata: Option<&str>,
) -> Result<compact::Report, keel::adapt::Error> {
    let plan = plan::build(tx, draft.strand, draft.first, draft.last).await?;
    if let Some(expected) = draft.expected
        && (expected.compact != draft.tag || !plan.matches(expected))
    {
        return Err(keel::adapt::Error::Adapt(
            "compact plan conflicts with its preview; retry compact".into(),
        ));
    }
    for (key, _) in &plan.absorbed {
        tx.end("Compact", *key).await?;
    }
    let strand = plan.strand.to_string();
    let first = plan.first.to_string();
    let last = plan.last.to_string();
    let mut fields = vec![
        ("tag", draft.tag),
        ("summary", draft.summary),
        ("created", draft.created),
        ("strand", strand.as_str()),
        ("head", first.as_str()),
        ("tail", last.as_str()),
    ];
    if let Some(metadata) = metadata {
        fields.push(("metadata", metadata));
    }
    crate::store::write::compact(tx, draft.tag, &fields).await?;
    Ok(plan.report(draft.tag, false))
}

impl Store {
    pub async fn selection(
        &self,
        strand: &str,
        request: &compact::Exec,
        limits: Limits,
    ) -> Result<compact::Choice, String> {
        self.core
            .batch(async |tx| Ok(scope::load(tx, strand).await?.select(request, limits)))
            .await
            .map_err(read::error)?
    }

    pub async fn settled(&self, strand: &str) -> Result<Option<compact::Range>, String> {
        self.core
            .batch(async |tx| Ok(scope::load(tx, strand).await?.settled().ok()))
            .await
            .map_err(read::error)
    }
}

pub(in crate::store) async fn selected(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    strand: &str,
    request: &compact::Exec,
    limits: Limits,
) -> Result<Result<compact::Choice, String>, keel::adapt::Error> {
    Ok(scope::load(tx, strand).await?.select(request, limits))
}
