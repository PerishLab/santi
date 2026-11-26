use crate::cli::Compact;

use super::super::{Capsule, Defaults, Http, Result, capsule, encode};

pub(in crate::client) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &Defaults,
    command: Compact,
) -> Result<()> {
    match command {
        Compact::Exec {
            first,
            last,
            summary,
            file,
            absorb,
        } => {
            let strand = defaults.strand(None)?;
            let summary = match file {
                Some(path) => crate::text::source::summary(&path)?,
                None => summary.expect("clap requires summary or summary_file"),
            };
            let mut body = serde_json::json!({ "summary": summary });
            if !absorb.is_empty() {
                body["absorb"] = serde_json::Value::from(absorb);
            }
            if let (Some(first), Some(last)) = (first, last) {
                body["first"] = serde_json::Value::from(first);
                body["last"] = serde_json::Value::from(last);
            }
            if let Some(soul) = defaults.soul() {
                body["soul"] = serde_json::Value::from(soul);
            }
            http.post(
                &format!("{base}/api/v1/strands/{strand}/compact"),
                Some(body),
            )
            .await
        }
        Compact::Capsule {
            first,
            last,
            from,
            to,
            summary,
            file,
            source,
            reason,
            risk,
            queryability,
            dry,
        } => {
            let body = capsule(Capsule {
                first,
                last,
                from,
                to,
                summary,
                file,
                source,
                reason,
                risk,
                queryability,
                preview: dry,
                soul: defaults.soul(),
            })?;
            let strand = defaults.strand(None)?;
            http.post(
                &format!("{base}/api/v1/strands/{strand}/compact"),
                Some(body),
            )
            .await
        }
        Compact::Query {
            compact,
            keyword,
            index,
            size,
        } => {
            let mut url =
                format!("{base}/api/v1/compacts/{compact}?page_index={index}&page_size={size}");
            if let Some(keyword) = keyword.filter(|k| !k.is_empty()) {
                url.push_str(&format!("&keyword={}", encode(&keyword)));
            }
            http.get(&url).await
        }
    }
}
