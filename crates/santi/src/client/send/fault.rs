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
