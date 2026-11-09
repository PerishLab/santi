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
    pub async fn job_query(
        &self,
        soul: &str,
        request: job::QueryRequest,
    ) -> Result<job::Query, job::QueryError> {
        let tree = self.shape(soul, request).await?;
        let keys = self.keys(soul).await.map_err(job::QueryError::Failed)?;
        if keys.is_empty() {
            return Ok(if tree.tally() {
                job::Query::Count(job::QueryCount { count: 0 })
            } else {
                job::Query::Rows(job::QueryRows { rows: Vec::new() })
            });
        }
        let refs = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let rows = self
            .core
            .ask(&tree.any("strand", &refs))
            .await
            .map_err(fault)?;
        if let Some(count) = rows.count() {
            return Ok(job::Query::Count(job::QueryCount { count }));
        }
        let projection = projection::Projection::new(self);
        let mut jobs = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            jobs.push(
                projection
                    .decode(row)
                    .await
                    .map_err(job::QueryError::Failed)?
                    .job,
            );
        }
        Ok(job::Query::Rows(job::QueryRows { rows: jobs }))
    }

    async fn shape(&self, soul: &str, request: job::QueryRequest) -> Result<Tree, job::QueryError> {
        if let Some(q) = request.q {
            let scoped = request.strand.is_some() || request.all_strands;
            let filtered = !request.state.is_empty() || request.limit.is_some();
            if scoped || filtered || request.count {
                return Err(job::QueryError::Invalid(
                    "q cannot be combined with structured query fields".to_string(),
                ));
            }
            return native(&q);
        }
        structured(self, soul, request).await
    }
}

fn native(text: &str) -> Result<Tree, job::QueryError> {
    let tree =
        parse(text).map_err(|error| job::QueryError::Invalid(format!("invalid query: {error}")))?;
    if !tree.from().eq_ignore_ascii_case("Job") {
        return Err(job::QueryError::Invalid(
            "query must select from Job".to_string(),
        ));
    }
    if !tree.links().is_empty() {
        return Err(job::QueryError::Invalid(
            "query links are not supported".to_string(),
        ));
    }
    if tree.after().is_some() {
        return Err(job::QueryError::Invalid(
            "query pagination is not supported".to_string(),
        ));
    }
    if tree.tally() && tree.limit().is_some() {
        return Err(job::QueryError::Invalid(
            "a count cannot carry a row limit".to_string(),
        ));
    }
    if !tree.tally() && tree.limit().is_none() {
        return Err(job::QueryError::Invalid(
            "query must declare limit or count".to_string(),
        ));
    }
    if let Some(limit) = tree.limit()
        && !(1..=CAP).contains(&limit)
    {
        return Err(job::QueryError::Invalid(format!(
            "query limit must be between 1 and {CAP}"
        )));
    }
    for pred in tree.preds() {
        if !PUBLIC.contains(&pred.field()) {
            return Err(job::QueryError::Invalid(format!(
                "field {} is not queryable",
                pred.field()
            )));
        }
        if matches!(pred.op(), Op::Has | Op::Some) {
            return Err(job::QueryError::Invalid(format!(
                "operator on {} is not supported",
                pred.field()
            )));
        }
        if pred.nest().is_some() {
            return Err(job::QueryError::Invalid(
                "nested query predicates are not supported".to_string(),
            ));
        }
    }
    for sort in tree.sorts() {
        if !PUBLIC.contains(&sort.field()) {
            return Err(job::QueryError::Invalid(format!(
                "field {} is not sortable",
                sort.field()
            )));
        }
    }
    Ok(tree)
}

async fn structured(
    store: &Store,
    soul: &str,
    request: job::QueryRequest,
) -> Result<Tree, job::QueryError> {
    if request.strand.is_some() && request.all_strands {
        return Err(job::QueryError::Invalid(
            "strand and all_strands cannot be combined".to_string(),
        ));
    }
    if request.strand.is_none() && !request.all_strands {
        return Err(job::QueryError::Invalid(
            "strand context required; pass a strand or all_strands".to_string(),
        ));
    }
    if request.count && request.limit.is_some() {
        return Err(job::QueryError::Invalid(
            "a count cannot carry a row limit".to_string(),
        ));
    }
    let limit = request.limit.unwrap_or(LIMIT);
    if !(1..=CAP).contains(&limit) {
        return Err(job::QueryError::Invalid(format!(
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
        let Some(key) = store
            .strand_key(soul, &tag)
            .await
            .map_err(job::QueryError::Failed)?
        else {
            return Err(job::QueryError::Invalid(
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

fn valid(state: &str) -> Result<(), job::QueryError> {
    if STATES.contains(&state) {
        Ok(())
    } else {
        Err(job::QueryError::Invalid(format!(
            "unknown job state {state}"
        )))
    }
}

fn fault(error: keel::adapt::Error) -> job::QueryError {
    match &error {
        keel::adapt::Error::Adapt(_)
        | keel::adapt::Error::Missing(_)
        | keel::adapt::Error::Estate(keel::estate::Fault::Denied { .. }) => {
            job::QueryError::Invalid(error.to_string())
        }
        _ => job::QueryError::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
