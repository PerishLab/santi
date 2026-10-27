use santi_provider::Call;

use super::Service;

pub(super) async fn rejected(
    service: &Service,
    call: &Call,
    error: String,
    limit: Option<usize>,
) -> Result<crate::tool::Reply, String> {
    let error = curbed(error, limit);
    service
        .store
        .create_reply(santi_estate::ReplyDraft {
            tag: &crate::tag("result"),
            call: &call.call,
            output: None,
            error: Some(&error),
            created: &crate::now(),
        })
        .await
}

pub(in crate::service) use santi_estate::curbed;

pub(super) async fn expired(
    service: &Service,
    effect: &str,
    call: &str,
    mut output: serde_json::Value,
) -> Result<crate::tool::Reply, String> {
    output["effect"] = serde_json::json!(effect);
    let error = serde_json::json!({
        "timed_out": true,
        "message": "foreground shell deadline exceeded; process group interrupted; partial external effects remain possible; inspect the effect before retrying; use a detached Job for long waits",
        "output": output,
    }).to_string();
    service
        .store
        .uncertain(
            effect,
            santi_estate::ReplyDraft {
                tag: &crate::tag("result"),
                call,
                output: None,
                error: Some(&error),
                created: &crate::now(),
            },
        )
        .await
}
