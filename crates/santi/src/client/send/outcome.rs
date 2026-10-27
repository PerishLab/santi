use std::fmt;

#[derive(Debug)]
pub(crate) struct Unsettled {
    detail: String,
    pub(crate) receipt: Option<String>,
}

impl fmt::Display for Unsettled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for Unsettled {}

pub(crate) fn unsettled(detail: String) -> anyhow::Error {
    anyhow::Error::new(Unsettled {
        detail,
        receipt: None,
    })
}

pub(crate) fn pending(receipt: &str, detail: String) -> anyhow::Error {
    anyhow::Error::new(Unsettled {
        detail,
        receipt: Some(receipt.to_string()),
    })
}
