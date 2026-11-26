use keel::{Op, Rank, Tree, form, query::parse};
use santi_model::job;

use super::{Store, projection};

const PUBLIC: &[&str] = &[
    "tag",
    "description",
    "command",
    "cwd",
    "timeout_seconds",
    "output_limit_bytes",
    "remind_every_seconds",
    "state",
    "reason",
    "exit_code",
    "created",
    "updated",
    "accepted",
    "started",
    "last_reminded",
    "next_reminder",
    "finished",
    "acknowledged",
];

const STATES: &[&str] = &[
    "submitting",
    "accepted",
    "running",
    "cancelling",
    "succeeded",
    "failed",
    "timed_out",
    "cancelled",
    "unknown",
];

const LIMIT: usize = 20;
const CAP: usize = 200;

impl Store {
    pub async fn query(&self, soul: &str, request: job::Request) -> Result<job::Query, job::Fault> {
        let tree = self.shape(soul, request).await?;
        let keys = self.keys(soul).await.map_err(job::Fault::Failed)?;
        if keys.is_empty() {
            return Ok(if tree.tally() {
                job::Query::Count(job::Count { count: 0 })
            } else {
                job::Query::Rows(job::Rows { rows: Vec::new() })
            });
        }
        let refs = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let rows = self
            .core
            .ask(&tree.any("strand", &refs))
            .await
            .map_err(fault)?;
        if let Some(count) = rows.count() {
            return Ok(job::Query::Count(job::Count { count }));
        }
        let projection = projection::Projection::new(self);
        let mut jobs = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            jobs.push(
                projection
                    .decode(row)
                    .await
                    .map_err(job::Fault::Failed)?
                    .job,
            );
        }
        Ok(job::Query::Rows(job::Rows { rows: jobs }))
    }

    async fn shape(&self, soul: &str, request: job::Request) -> Result<Tree, job::Fault> {
        if let Some(q) = request.q {
            let scoped = request.strand.is_some() || request.all;
            let filtered = !request.state.is_empty() || request.limit.is_some();
            if scoped || filtered || request.count {
                return Err(job::Fault::Invalid(
                    "q cannot be combined with structured query fields".to_string(),
                ));
            }
            return native(&q);
        }
        structured(self, soul, request).await
    }
}

fn native(text: &str) -> Result<Tree, job::Fault> {
    let tree =
        parse(text).map_err(|error| job::Fault::Invalid(format!("invalid query: {error}")))?;
    if !tree.from().eq_ignore_ascii_case("Job") {
        return Err(job::Fault::Invalid(
            "query must select from Job".to_string(),
        ));
    }
    if !tree.links().is_empty() {
        return Err(job::Fault::Invalid(
            "query links are not supported".to_string(),
        ));
    }
    if tree.after().is_some() {
        return Err(job::Fault::Invalid(
            "query pagination is not supported".to_string(),
        ));
    }
    if tree.tally() && tree.limit().is_some() {
        return Err(job::Fault::Invalid(
            "a count cannot carry a row limit".to_string(),
        ));
    }
    if !tree.tally() && tree.limit().is_none() {
        return Err(job::Fault::Invalid(
            "query must declare limit or count".to_string(),
        ));
    }
    if let Some(limit) = tree.limit()
        && !(1..=CAP).contains(&limit)
    {
        return Err(job::Fault::Invalid(format!(
            "query limit must be between 1 and {CAP}"
        )));
    }
    for pred in tree.preds() {
        if !PUBLIC.contains(&pred.field()) {
            return Err(job::Fault::Invalid(format!(
                "field {} is not queryable",
                pred.field()
            )));
        }
        if matches!(pred.op(), Op::Has | Op::Some) {
            return Err(job::Fault::Invalid(format!(
                "operator on {} is not supported",
                pred.field()
            )));
        }
        if pred.nest().is_some() {
            return Err(job::Fault::Invalid(
                "nested query predicates are not supported".to_string(),
            ));
        }
    }
    for sort in tree.sorts() {
        if !PUBLIC.contains(&sort.field()) {
            return Err(job::Fault::Invalid(format!(
                "field {} is not sortable",
                sort.field()
            )));
        }
    }
    Ok(tree)
}

async fn structured(store: &Store, soul: &str, request: job::Request) -> Result<Tree, job::Fault> {
    if request.strand.is_some() && request.all {
        return Err(job::Fault::Invalid(
            "strand and all_strands cannot be combined".to_string(),
        ));
    }
    if request.strand.is_none() && !request.all {
        return Err(job::Fault::Invalid(
            "strand context required; pass a strand or all_strands".to_string(),
        ));
    }
    if request.count && request.limit.is_some() {
        return Err(job::Fault::Invalid(
            "a count cannot carry a row limit".to_string(),
        ));
    }
    let limit = request.limit.unwrap_or(LIMIT);
    if !(1..=CAP).contains(&limit) {
        return Err(job::Fault::Invalid(format!(
            "limit must be between 1 and {CAP}"
        )));
    }
    for state in &request.state {
        valid(state)?;
    }
    let mut tree = form("Job");
    if !request.state.is_empty() {
        let states = request.state.iter().map(String::as_str).collect::<Vec<_>>();
        tree = tree.any("state", &states);
    }
    if let Some(tag) = request.strand {
        let Some(key) = store.key(soul, &tag).await.map_err(job::Fault::Failed)? else {
            return Err(job::Fault::Invalid(
                "strand not found for this soul".to_string(),
            ));
        };
        tree = tree.when("strand", Op::Eq, &key);
    }
    if request.count {
        Ok(tree.count())
    } else {
        Ok(tree
            .order("created", Rank::Desc)
            .order("tag", Rank::Desc)
            .top(limit))
    }
}

fn valid(state: &str) -> Result<(), job::Fault> {
    if STATES.contains(&state) {
        Ok(())
    } else {
        Err(job::Fault::Invalid(format!("unknown job state {state}")))
    }
}

fn fault(error: keel::adapt::Error) -> job::Fault {
    match &error {
        keel::adapt::Error::Adapt(_)
        | keel::adapt::Error::Missing(_)
        | keel::adapt::Error::Estate(keel::estate::Fault::Denied { .. }) => {
            job::Fault::Invalid(error.to_string())
        }
        _ => job::Fault::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
