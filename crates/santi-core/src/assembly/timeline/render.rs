use santi_model::compact;
use serde_json::json;

pub(super) struct Range {
    pub(super) from: i64,
    pub(super) to: i64,
    pub(super) collapsed: i64,
}

pub(super) fn condensed(compact: &compact::Compact, fallback: Range) -> String {
    let metadata = compact.metadata.as_ref();
    let capsuled = metadata
        .and_then(|metadata| metadata.get("schema"))
        .and_then(|value| value.as_str())
        == Some("santi.compact_capsule.v1");
    let range = metadata.and_then(|metadata| metadata.get("range"));
    let from = number(range, "start_seq");
    let to = number(range, "end_seq");
    let collapsed = number(range, "collapsed_count");
    let header = json!({
        "schema": "santi.compact_projection.visible_header.v1",
        "compact": compact.id,
        "operation": noted(metadata, "operation")
            .unwrap_or(if capsuled { "manual_capsule" } else { "compact_projection" }),
        "declared_source": noted(metadata, "declared_source").unwrap_or("not_declared"),
        "source_trust": noted(metadata, "source_trust")
            .unwrap_or(if capsuled { "caller_declared" } else { "not_declared" }),
        "covered_message_range": {
            "first": compact.first,
            "last": compact.last,
        },
        "covered_strand_seq": {
            "start_seq": from.unwrap_or(fallback.from),
            "end_seq": to.unwrap_or(fallback.to),
        },
        "collapsed_entries": collapsed.unwrap_or(fallback.collapsed),
        "reason": noted(metadata, "reason").unwrap_or("not_declared"),
        "risk": noted(metadata, "risk").unwrap_or("not_declared"),
        "queryability": noted(metadata, "queryability")
            .unwrap_or("original spine remains queryable with compact query"),
        "originals_query": format!("santi compact query --compact {}", compact.id),
        "context_estimate": estimate(metadata),
    });
    let header = serde_json::to_string_pretty(&header).unwrap_or_else(|_| header.to_string());
    format!(
        "[compact projection]\n{header}\n[/compact projection]\n<compact_summary>\n{}\n</compact_summary>",
        compact.summary
    )
}

fn estimate(metadata: Option<&serde_json::Value>) -> serde_json::Value {
    let before = nested(metadata, "before", "total");
    let budget = nested(metadata, "budget", "bytes");
    let forecast = metadata
        .and_then(|value| value.get("forecast"))
        .filter(|value| value.is_object());
    let after = nested(forecast, "after", "total");
    let ratio = forecast
        .and_then(|value| value.get("ratio"))
        .and_then(|value| value.as_f64());
    let bound = match (budget, metadata.and_then(|value| value.get("budget"))) {
        (Some(_), _) => "bounded",
        (None, Some(serde_json::Value::Null)) => "unbounded",
        (None, Some(_)) => "unavailable",
        (None, None) => "not_recorded",
    };
    let mut value = json!({
        "pre_total_bytes": before,
        "budget_input_bytes": budget,
        "availability": {
            "pre": availability(metadata, "before", before.is_some()),
            "budget": bound,
            "forecast": availability(metadata, "forecast", after.is_some()),
        },
    });
    if let Some(forecast) = forecast {
        let mut prediction = json!({
            "authoritative": forecast.get("authoritative").and_then(|value| value.as_bool()),
            "basis": noted(Some(forecast), "basis"),
            "post_total_bytes": after,
            "ratio": ratio,
            "ratio_state": if ratio.is_some() { "estimated" } else if before.is_some_and(|value| value <= 0) { "undefined" } else { "not_recorded" },
        });
        omit(&mut prediction);
        value["forecast"] = prediction;
    }
    omit(&mut value);
    value
}

fn availability(metadata: Option<&serde_json::Value>, key: &str, present: bool) -> &'static str {
    if present {
        return "estimated";
    }
    if metadata.is_some_and(|value| value.get(key).is_some()) {
        "unavailable"
    } else {
        "not_recorded"
    }
}

fn omit(value: &mut serde_json::Value) {
    if let Some(object) = value.as_object_mut() {
        object.retain(|_, value| !value.is_null());
    }
}

fn number(value: Option<&serde_json::Value>, key: &str) -> Option<i64> {
    value
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_i64())
}

fn nested(value: Option<&serde_json::Value>, outer: &str, inner: &str) -> Option<i64> {
    value
        .and_then(|value| value.get(outer))
        .and_then(|value| value.get(inner))
        .and_then(|value| value.as_i64())
}

fn noted<'a>(metadata: Option<&'a serde_json::Value>, key: &str) -> Option<&'a str> {
    metadata
        .and_then(|metadata| metadata.get(key))
        .and_then(|value| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing() {
        let legacy = estimate(None);
        assert_eq!(legacy["availability"]["pre"], "not_recorded");
        assert_eq!(legacy["availability"]["budget"], "not_recorded");
        assert_eq!(legacy["availability"]["forecast"], "not_recorded");
        assert!(legacy.get("pre_total_bytes").is_none());
        assert!(legacy.get("budget_input_bytes").is_none());
        assert!(legacy.get("forecast").is_none());
        let unavailable = json!({"before":null,"budget":null,"forecast":null});
        let value = estimate(Some(&unavailable));
        assert_eq!(value["availability"]["pre"], "unavailable");
        assert_eq!(value["availability"]["budget"], "unbounded");
        assert_eq!(value["availability"]["forecast"], "unavailable");
    }

    #[test]
    fn zero() {
        let forecast = json!({"authoritative":false,"basis":"precommit_preview","after":{"total":0},"ratio":0.0});
        let metadata = json!({"before":{"total":0},"budget":{"bytes":0},"forecast":forecast});
        let value = estimate(Some(&metadata));
        assert_eq!(value["pre_total_bytes"], 0);
        assert_eq!(value["budget_input_bytes"], 0);
        assert_eq!(value["forecast"]["post_total_bytes"], 0);
        assert_eq!(value["forecast"]["ratio"], 0.0);
        assert_eq!(value["availability"]["pre"], "estimated");
        assert_eq!(value["availability"]["forecast"], "estimated");
        let undefined =
            json!({"before":{"total":0},"forecast":{"authoritative":false,"after":{"total":1}}});
        let value = estimate(Some(&undefined));
        assert!(value["forecast"].get("ratio").is_none());
        assert_eq!(value["forecast"]["ratio_state"], "undefined");
    }

    #[test]
    fn retained() {
        let compact = compact::Compact {
            id: "compact_current".into(),
            strand: "strand_test".into(),
            summary: "retained summary".into(),
            first: "message_first".into(),
            last: "message_last".into(),
            created: None,
            metadata: Some(
                json!({"originals_query":"santi compact query --compact-id compact_wrong"}),
            ),
        };
        let rendered = condensed(
            &compact,
            Range {
                from: 1,
                to: 5,
                collapsed: 5,
            },
        );
        let text = rendered
            .split_once("[compact projection]\n")
            .unwrap()
            .1
            .split_once("\n[/compact projection]")
            .unwrap()
            .0;
        let header: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            header["originals_query"],
            "santi compact query --compact compact_current"
        );
        assert_eq!(header["covered_strand_seq"]["end_seq"], 5);
        assert!(rendered.contains("<compact_summary>\nretained summary"));
        assert_eq!(
            compact.metadata.unwrap()["originals_query"],
            "santi compact query --compact-id compact_wrong"
        );
    }
}
