use santi_provider::Call;

use crate::compact;
use crate::service::Service;

use super::super::reply;

impl Service {
    pub(in crate::service::tools) async fn compacted(
        &self,
        strand: &str,
        call: &Call,
        limit: Option<usize>,
    ) -> Result<crate::tool::Reply, String> {
        let result = match asked(call) {
            Ok(request) => self.exec(strand, request).await,
            Err(error) => Err(error),
        };
        match result {
            Ok(report) => {
                let output = serde_json::to_value(report).map_err(|error| error.to_string())?;
                self.store
                    .create_reply(santi_estate::ReplyDraft {
                        tag: &crate::tag("result"),
                        call: &call.call,
                        output: Some(&output),
                        error: None,
                        created: &crate::now(),
                    })
                    .await
            }
            Err(error) => reply::rejected(self, call, error, limit).await,
        }
    }
}

fn asked(call: &Call) -> Result<compact::Exec, String> {
    let value = &call.arguments;
    let summary = value
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "compact requires a summary".to_string())?
        .to_string();
    Ok(compact::Exec {
        first: text(value, "first"),
        last: text(value, "last"),
        from: None,
        to: None,
        summary,
        absorb: absorbed(value),
        capsule: None,
        dry: false,
    })
}

fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.trim().is_empty())
        .map(str::to_string)
}

fn absorbed(value: &serde_json::Value) -> Vec<String> {
    value
        .get("absorb")
        .and_then(serde_json::Value::as_array)
        .map(|held| {
            held.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
