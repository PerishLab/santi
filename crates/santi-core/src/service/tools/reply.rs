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
