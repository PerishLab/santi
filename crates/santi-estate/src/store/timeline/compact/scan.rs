use super::thread::{Call, Frame, Recorded};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Rank, Row, Tx, form};
use std::collections::BTreeMap;

pub(super) struct Scan<'a, 'tx>(pub(super) &'a mut Tx<'tx, Sqlite>);

impl<'a, 'tx> Scan<'a, 'tx> {
    pub(super) async fn frames(&mut self, strand: &str) -> Result<Vec<Frame>, Error> {
        let mut frames = Vec::new();
        for entry in self.ordered(strand, "thinking").await? {
            let at = number(&entry, "sequence")?;
            let key = entry
                .int("thinking")
                .ok_or_else(|| Error::Adapt("thinking entry target missing".into()))?;
            let span = self
                .0
                .one(&form("ThinkingSpan").when("id", Op::Eq, &key.to_string()))
                .await?
                .ok_or_else(|| Error::Missing(format!("ThinkingSpan {key}")))?;
            let turn = number(&span, "turn")?;
            let round = match self.recorded(key).await? {
                Some(row) => decode(&row),
                None => Recorded::Legacy,
            };
            frames.push(Frame { at, turn, round });
        }
        Ok(frames)
    }

    pub(super) async fn calls(&mut self, strand: &str) -> Result<Vec<Call>, Error> {
        let mut calls = Vec::new();
        for entry in self.ordered(strand, "tool_call").await? {
            let at = number(&entry, "sequence")?;
            let key = entry
                .int("call")
                .ok_or_else(|| Error::Adapt("tool call entry target missing".into()))?;
            let call = self
                .0
                .one(&form("ToolCall").when("id", Op::Eq, &key.to_string()))
                .await?
                .ok_or_else(|| Error::Missing(format!("ToolCall {key}")))?;
            calls.push(Call {
                at,
                key,
                tool: text(&call, "tool")?.to_string(),
                turn: number(&call, "turn")?,
            });
        }
        Ok(calls)
    }

    pub(super) async fn results(
        &mut self,
        strand: &str,
    ) -> Result<BTreeMap<i64, (i64, usize)>, Error> {
        let mut ends = BTreeMap::new();
        for entry in self.ordered(strand, "tool_result").await? {
            let at = number(&entry, "sequence")?;
            let key = entry
                .int("result")
                .ok_or_else(|| Error::Adapt("tool result entry target missing".into()))?;
            let result = self
                .0
                .one(&form("ToolResult").when("id", Op::Eq, &key.to_string()))
                .await?
                .ok_or_else(|| Error::Missing(format!("ToolResult {key}")))?;
            let call = number(&result, "call")?;
            let held = ends.entry(call).or_insert((at, 0));
            held.0 = held.0.max(at);
            held.1 += 1;
        }
        Ok(ends)
    }

    pub(super) async fn locate(
        &mut self,
        strand: &str,
        from: i64,
        to: Option<i64>,
    ) -> Result<Option<i64>, Error> {
        let query = form("StrandEntry")
            .when("strand", Op::Eq, strand)
            .when("target_type", Op::Eq, "message")
            .when("sequence", Op::Gt, &from.to_string());
        let query = match to {
            Some(to) => query.when("sequence", Op::Lt, &to.to_string()),
            None => query,
        };
        let rows = self
            .0
            .ask(&query.order("sequence", Rank::Asc))
            .await?
            .rows()
            .to_vec();
        for entry in rows {
            let key = entry
                .int("message")
                .ok_or_else(|| Error::Adapt("message entry target missing".into()))?;
            let message = self
                .0
                .one(&form("Message").when("id", Op::Eq, &key.to_string()))
                .await?
                .ok_or_else(|| Error::Missing(format!("Message {key}")))?;
            if message.text("actor_type") == Some("soul") && message.text("kind") == Some("text") {
                return Ok(entry.int("sequence"));
            }
        }
        Ok(None)
    }

    async fn recorded(&mut self, key: i64) -> Result<Option<Row>, Error> {
        self.0
            .one(&form("Round").when("thinking", Op::Eq, &key.to_string()))
            .await
    }

    async fn ordered(&mut self, strand: &str, kind: &str) -> Result<Vec<Row>, Error> {
        Ok(self
            .0
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, strand)
                    .when("target_type", Op::Eq, kind)
                    .order("sequence", Rank::Asc),
            )
            .await?
            .rows()
            .to_vec())
    }
}

fn decode(row: &Row) -> Recorded {
    let Some(record) = row.text("record") else {
        return Recorded::Broken;
    };
    serde_json::from_str(record).map_or(Recorded::Broken, |round| Recorded::Round(Box::new(round)))
}

fn text<'a>(row: &'a Row, field: &str) -> Result<&'a str, Error> {
    row.text(field)
        .ok_or_else(|| Error::Adapt(format!("compact {field} missing")))
}

fn number(row: &Row, field: &str) -> Result<i64, Error> {
    row.int(field)
        .ok_or_else(|| Error::Adapt(format!("compact {field} missing")))
}
