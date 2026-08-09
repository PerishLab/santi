use std::fmt;

#[derive(Debug)]
pub(crate) struct Unsettled(String);

impl fmt::Display for Unsettled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unsettled {}

pub(crate) fn unsettled(detail: String) -> anyhow::Error {
    anyhow::Error::new(Unsettled(detail))
}
