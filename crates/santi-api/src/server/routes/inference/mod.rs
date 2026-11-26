use super::*;

pub(crate) mod changes;
pub(crate) mod soul;
pub(crate) mod sources;
pub(crate) mod strand;

pub(super) fn reject(error: String) -> Problem {
    if error.contains("inference rejected:") {
        Problem::invalid(error)
    } else {
        Problem::service(error)
    }
}

#[cfg(test)]
mod tests;
