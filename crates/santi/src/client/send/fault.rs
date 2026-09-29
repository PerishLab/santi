use super::outcome::unsettled;

pub(crate) fn unknown(strand: &str, detail: String) -> anyhow::Error {
    unsettled(format!(
        "strand send outcome=state_unknown: {detail}; the message may have been accepted, but its exact receipt was not proven; do not resend it; inspect with `santi strand runtime {strand}` and recover the accepted message's receipt/status before resuming or explicitly redriving with `santi strand drive {strand}`"
    ))
}

pub(crate) fn uncertain(strand: &str, receipt: &str, detail: String) -> anyhow::Error {
    unsettled(format!(
        "watch outcome=state_unknown: {detail} for accepted receipt {receipt}; do not resend the accepted message; inspect with `santi receipt {receipt}` and `santi strand runtime {strand}`; resume the blocking condition or explicitly redrive with `santi strand drive {strand}`"
    ))
}

pub(crate) fn accepted_warning(value: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    value?
        .pointer("/receipt/warning")
        .filter(|warning| !warning.is_null())
}

pub(crate) fn accepted_warning_error(warning: &serde_json::Value, receipt: &str) -> anyhow::Error {
    let detail = if let Some(command) = warning
        .pointer("/context/recovery/command")
        .and_then(serde_json::Value::as_str)
    {
        format!(
            "message was accepted as exact receipt {receipt} but not driven; do not resend it; run `{command}`"
        )
    } else {
        let code = warning
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown warning");
        format!(
            "message was accepted as exact receipt {receipt} but not driven; do not resend it; inspect and resolve `{code}`"
        )
    };
    unsettled(detail)
}

#[cfg(test)]
mod tests {
    use super::super::outcome::Unsettled;
    use super::{accepted_warning_error, uncertain, unknown};

    #[test]
    fn typed() {
        let warning = serde_json::json!({
            "code": "runtime.strand.drive_failed",
            "context": { "recovery": { "command": "santi strand drive ss_direct" } }
        });
        let errors = [
            unknown("ss_direct", "POST response was ambiguous".to_string()),
            uncertain(
                "ss_direct",
                "inbox_direct",
                "receipt was unreadable".to_string(),
            ),
            accepted_warning_error(&warning, "inbox_direct"),
        ];

        for error in errors {
            assert!(error.downcast_ref::<Unsettled>().is_some());
            assert!(error.to_string().contains("do not resend"));
        }
    }
}
