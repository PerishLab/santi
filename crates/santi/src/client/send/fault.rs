use super::outcome::{pending, unsettled};
use super::{Target, guidance};

pub(crate) fn unknown(target: Target<'_>, detail: String) -> anyhow::Error {
    unsettled(format!(
        "strand send outcome=state_unknown: {detail}; the message may have been accepted, but its exact receipt was not proven; do not resend it; {} and recover the accepted message's receipt/status before resuming or {}",
        guidance::inspect(target.base, target.strand, None),
        guidance::drive(target.base, target.strand)
    ))
}

pub(crate) fn uncertain(target: Target<'_>, receipt: &str, detail: String) -> anyhow::Error {
    pending(
        receipt,
        format!(
            "watch outcome=state_unknown: {detail} for accepted receipt {receipt}; do not resend the accepted message; {}; resume the blocking condition or {}",
            guidance::inspect(target.base, target.strand, Some(receipt)),
            guidance::drive(target.base, target.strand)
        ),
    )
}

pub(crate) fn accepted_warning(value: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    value?
        .pointer("/receipt/warning")
        .filter(|warning| !warning.is_null())
}

pub(crate) fn accepted_warning_error(
    target: Target<'_>,
    warning: &serde_json::Value,
    receipt: &str,
) -> anyhow::Error {
    let code = warning
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown warning");
    pending(
        receipt,
        format!(
            "message was accepted as exact receipt {receipt} but not driven; do not resend it; {}; inspect and resolve `{code}` before resuming or {}",
            guidance::inspect(target.base, target.strand, Some(receipt)),
            guidance::drive(target.base, target.strand)
        ),
    )
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
        let client = reqwest::Client::new();
        let target = super::Target::tui(&client, "http://localhost:43309", "ss_direct");
        let errors = [
            unknown(target, "POST response was ambiguous".to_string()),
            uncertain(target, "inbox_direct", "receipt was unreadable".to_string()),
            accepted_warning_error(target, &warning, "inbox_direct"),
        ];

        for error in errors {
            assert!(error.downcast_ref::<Unsettled>().is_some());
            assert!(error.to_string().contains("do not resend"));
        }
    }
}
