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

#[derive(Debug)]
pub(crate) struct Failed {
    pub(crate) receipt: String,
    pub(crate) turn: Option<String>,
}

impl fmt::Display for Failed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let receipt = &self.receipt;
        write!(
            formatter,
            "strand send outcome=failed: accepted receipt {receipt} reached its durable failed state; do not resend the accepted message; inspect with `santi receipt {receipt}`"
        )
    }
}

impl std::error::Error for Failed {}
