use santi_provider::Call;

use super::{Service, curbed};

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
