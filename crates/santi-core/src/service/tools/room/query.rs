use santi_provider::{Call, Function, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::service::{Service, tools::argued};

pub(in crate::service::tools) fn definition() -> Tool {
    Tool::Function(Function {
        name: "query".into(),
        description: "Read this strand's memory. With no arguments, list occupied compact slots and default settled boundaries. Use kind messages to page exact Message IDs, then kind content with a Message or Compact ID to read text in byte chunks. Results carry continuation offsets. Available during context maintenance.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["slots", "messages", "content"]},
                "id": {"type": "string", "description": "Message or Compact ID for content."},
                "offset": {"type": "integer", "minimum": 0, "description": "Returned continuation: message sequence for messages, byte offset for content."}
            },
            "additionalProperties": false
        }),
    })
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    #[default]
    Slots,
    Messages,
    Content,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Args {
    kind: Kind,
    id: Option<String>,
    offset: usize,
}

impl Service {
    pub(in crate::service) async fn queried(
        &self,
        strand: &str,
        call: &Call,
    ) -> Result<Value, String> {
        let args: Args = argued(&call.arguments)?;
        match args.kind {
            Kind::Slots => self.slots(strand).await,
            Kind::Messages => self.headers(strand, args.offset).await,
            Kind::Content => self.content(strand, args).await,
        }
    }

    async fn slots(&self, strand: &str) -> Result<Value, String> {
        let spans = self.spans(strand).await?;
        let mut slots = Vec::new();
        for compact in self.store.compacts(strand).await? {
            if let Some(span) = spans.iter().find(|span| span.compact == compact.id)
                && !crate::service::face::compact::slots::covered(span, &spans)
            {
                slots.push(json!({"id": compact.id, "first": compact.first, "last": compact.last, "from": span.from, "to": span.to, "bytes": compact.summary.len()}));
            }
        }
        slots.sort_by_key(|slot| slot["from"].as_i64());
        let default = self.store.settled(strand).await?;
        let policy = self.regimen();
        Ok(
            json!({"slots": slots, "capacity": policy.slots, "default": default, "read": "query kind content with an id; compact summary alone uses default, absorb uses occupied slot ids"}),
        )
    }

    async fn headers(&self, strand: &str, offset: usize) -> Result<Value, String> {
        let messages = self.store.messages(strand).await?;
        let mut remaining = messages
            .iter()
            .filter(|message| message.relation.seq > i64::try_from(offset).unwrap_or(i64::MAX));
        let page = remaining.by_ref().take(16).map(|message| json!({"id": message.message.id, "sequence": message.relation.seq, "state": message.message.state, "bytes": message.message.content.rendered().len()})).collect::<Vec<_>>();
        let next = remaining
            .next()
            .and_then(|_| page.last().and_then(|message| message["sequence"].as_i64()));
        Ok(json!({"messages": page, "next": next}))
    }

    async fn content(&self, strand: &str, args: Args) -> Result<Value, String> {
        let id = args.id.as_deref().ok_or_else(|| {
            "query content requires id; query slots or messages for current references".to_string()
        })?;
        let text = if let Some(compact) = self.store.compact(id).await? {
            if compact.strand != strand {
                return Err("query id is not in this strand".into());
            }
            compact.summary
        } else {
            self.store
                .messages(strand)
                .await?
                .into_iter()
                .find(|message| message.message.id == id)
                .map(|message| message.message.content.rendered())
                .ok_or_else(|| {
                    "query id is not in this strand; query slots or messages for current references"
                        .to_string()
                })?
        };
        if args.offset > text.len() || !text.is_char_boundary(args.offset) {
            return Err("query offset must be a UTF-8 boundary within the content".into());
        }
        let mut end = args.offset.saturating_add(8192).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        Ok(
            json!({"id": id, "offset": args.offset, "bytes": text.len(), "text": &text[args.offset..end], "next": (end < text.len()).then_some(end)}),
        )
    }

    pub(in crate::service) async fn read(
        &self,
        strand: &str,
        call: &Call,
        limit: Option<usize>,
    ) -> Result<crate::tool::Reply, String> {
        match self.queried(strand, call).await {
            Ok(output) => {
                self.store
                    .create_reply(santi_estate::ReplyDraft {
                        tag: &crate::tag("result"),
                        call: &call.call,
                        output: Some(&santi_estate::bounded(output, limit)),
                        error: None,
                        created: &crate::now(),
                    })
                    .await
            }
            Err(error) => crate::service::tools::reply::rejected(self, call, error, limit).await,
        }
    }
}

#[cfg(test)]
#[path = "inspection.rs"]
mod tests;
