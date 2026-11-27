use super::super::target;
use super::boundary;
use keel::adapt::Error;
use keel::adapt::db::Sqlite;
use keel::{Op, Rank, Row, Tx, form};
use santi_model::compact;

pub(super) struct Query<'a> {
    pub tag: &'a str,
    pub keyword: Option<&'a str>,
    pub index: i64,
    pub size: i64,
}

pub(super) struct Reader<'a, 'tx>(pub(super) &'a mut Tx<'tx, Sqlite>);

impl Reader<'_, '_> {
    pub(super) async fn read(&mut self, query: Query<'_>) -> Result<compact::Lookup, Error> {
        let Some(row) = self
            .0
            .one(&form("Compact").when("tag", Op::Eq, query.tag))
            .await?
        else {
            let Some(retired) = self
                .0
                .one(&form("RetiredCompact").when("tag", Op::Eq, query.tag))
                .await?
            else {
                return Ok(compact::Lookup::Missing);
            };
            let key = boundary::number(&retired, "retirement")?.to_string();
            let retirement = self.need("Retirement", "id", &key).await?;
            return Ok(compact::Lookup::Retired {
                strand: text(&retirement, "tag")?.into(),
            });
        };
        let key = boundary::number(&row, "strand")?.to_string();
        let strand = self.need("Strand", "id", &key).await?;
        let (head, tail) = boundary::Reader(self.0).ends(&row).await?;
        let first = boundary::Reader(self.0).tag(&head).await?;
        let last = boundary::Reader(self.0).tag(&tail).await?;
        let from = boundary::number(&head, "sequence")?;
        let to = boundary::number(&tail, "sequence")?;
        let entries = self.entries(&key, from, to).await?;
        let needle = query
            .keyword
            .map(str::trim)
            .filter(|keyword| !keyword.is_empty())
            .map(str::to_lowercase);
        let entries = entries
            .into_iter()
            .filter(|entry| {
                needle
                    .as_ref()
                    .is_none_or(|needle| entry.text.to_lowercase().contains(needle))
            })
            .collect::<Vec<_>>();
        let total = entries.len() as i64;
        let skip = query.index.max(0).saturating_mul(query.size.max(0)) as usize;
        let take = query.size.max(0) as usize;
        Ok(compact::Lookup::Live(compact::Page {
            compact: text(&row, "tag")?.into(),
            strand: text(&strand, "tag")?.into(),
            first,
            last,
            total,
            index: query.index,
            size: query.size,
            entries: entries.into_iter().skip(skip).take(take).collect(),
        }))
    }

    async fn entries(
        &mut self,
        strand: &str,
        from: i64,
        to: i64,
    ) -> Result<Vec<compact::Entry>, Error> {
        let rows = self
            .0
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, strand)
                    .when("sequence", Op::Ge, &from.to_string())
                    .when("sequence", Op::Le, &to.to_string())
                    .order("sequence", Rank::Asc),
            )
            .await?
            .rows()
            .to_vec();
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let kind = text(&row, "target_type")?;
            let aimed = crate::store::entry::aim(self.0, &row).await?;
            let target = text(&aimed, "tag")?;
            entries.push(compact::Entry {
                seq: row
                    .int("sequence")
                    .ok_or_else(|| Error::Adapt("entry sequence missing".into()))?,
                kind: target::decode(kind).map_err(Error::Adapt)?,
                target: target.to_string(),
                text: self.render(kind, target).await?,
            });
        }
        Ok(entries)
    }

    async fn render(&mut self, kind: &str, tag: &str) -> Result<String, Error> {
        match kind {
            "message" => {
                let row = self.need("Message", "tag", tag).await?;
                let content = text(&row, "content")?;
                serde_json::from_str::<santi_model::message::Content>(content)
                    .map(|content| content.rendered())
                    .map_err(|error| Error::Adapt(error.to_string()))
            }
            "tool_call" => {
                let row = self.need("ToolCall", "tag", tag).await?;
                let arguments = json(text(&row, "arguments")?)?;
                Ok(format!(
                    "[tool_call {}] {}",
                    text(&row, "tool")?,
                    rendered(&arguments)
                ))
            }
            "tool_result" => self.result(tag).await,
            "thinking" => {
                let row = self.need("ThinkingSpan", "tag", tag).await?;
                Ok(row
                    .text("summary")
                    .map(|summary| format!("[thinking] {summary}"))
                    .unwrap_or_default())
            }
            _ => Ok(String::new()),
        }
    }

    async fn result(&mut self, tag: &str) -> Result<String, Error> {
        let result = self.need("ToolResult", "tag", tag).await?;
        let key = result.key().to_string();
        if let Some(output) = self
            .0
            .one(&form("ToolOutput").when("result", Op::Eq, &key))
            .await?
        {
            return Ok(format!(
                "[tool_result] {}",
                rendered(&json(text(&output, "output")?)?)
            ));
        }
        if let Some(failure) = self
            .0
            .one(&form("ToolFailure").when("result", Op::Eq, &key))
            .await?
        {
            return Ok(format!("[tool_result error] {}", text(&failure, "error")?));
        }
        Ok("[tool_result]".to_string())
    }

    async fn need(&mut self, unit: &str, field: &str, value: &str) -> Result<Row, Error> {
        self.0
            .one(&form(unit).when(field, Op::Eq, value))
            .await?
            .ok_or_else(|| Error::Missing(format!("{unit} {value}")))
    }
}

fn json(value: &str) -> Result<serde_json::Value, keel::adapt::Error> {
    serde_json::from_str(value).map_err(|error| keel::adapt::Error::Adapt(error.to_string()))
}

fn rendered(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn text<'a>(row: &'a Row, field: &str) -> Result<&'a str, keel::adapt::Error> {
    row.text(field)
        .ok_or_else(|| Error::Adapt(format!("compact page {field} missing")))
}
