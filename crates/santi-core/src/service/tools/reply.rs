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

pub(in crate::service) fn curbed(error: String, limit: Option<usize>) -> String {
    let Some(limit) = limit else {
        return error;
    };
    if error.len() <= limit {
        return error;
    }
    let mut end = limit;
    while end > 0 && !error.is_char_boundary(end) {
        end -= 1;
    }
    error[..end].to_string()
}
