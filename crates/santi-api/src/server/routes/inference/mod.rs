use super::*;

pub(crate) mod soul;
pub(crate) mod strand;

pub(super) fn reject(error: String) -> ApiError {
    if error.contains("inference rejected:") {
        ApiError::bad_request(error)
    } else {
        ApiError::from_service(error)
    }
}

#[cfg(test)]
mod tests;
