use crate::cli::CompactCommand;
use crate::text::source::read_summary_file;

use super::super::{
    Capsule, ClientDefaults, Http, Result, compact_capsule_body, urlencoding_encode,
};

pub(in crate::client) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: CompactCommand,
) -> Result<()> {
    match command {
        CompactCommand::Exec {
            first,
            last,
            summary,
            summary_file,
            absorb,
        } => {
            let strand = defaults.resolve_strand(None)?;
            let summary = match summary_file {
                Some(path) => read_summary_file(&path)?,
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
        CompactCommand::Capsule {
            first,
            last,
            from,
            to,
            summary,
            summary_file,
            source,
            reason,
            risk,
            queryability,
            dry,
        } => {
            let body = compact_capsule_body(Capsule {
                first,
                last,
                from,
                to,
                summary,
                file: summary_file,
                source,
                reason,
                risk,
                queryability,
                preview: dry,
                soul: defaults.soul(),
            })?;
            let strand = defaults.resolve_strand(None)?;
            http.post(
                &format!("{base}/api/v1/strands/{strand}/compact"),
                Some(body),
            )
            .await
        }
        CompactCommand::Query {
            compact,
            keyword,
            page_index,
            page_size,
        } => {
            let mut url = format!(
                "{base}/api/v1/compacts/{compact}?page_index={page_index}&page_size={page_size}"
            );
            if let Some(keyword) = keyword.filter(|k| !k.is_empty()) {
                url.push_str(&format!("&keyword={}", urlencoding_encode(&keyword)));
            }
            http.get(&url).await
        }
    }
}
