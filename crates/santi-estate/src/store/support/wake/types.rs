#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub soul: String,
    pub generation: u64,
    pub state: String,
    pub remaining: u8,
    pub cadence: u64,
    pub next: Option<i64>,
    pub last: Option<i64>,
    pub created: String,
    pub updated: String,
}

#[derive(Clone, Copy)]
pub struct Wake<'a> {
    pub soul: &'a str,
    pub generation: u64,
    pub due: i64,
    pub now: i64,
    pub occurred: &'a str,
    pub notice: crate::store::Notice<'a>,
    pub gate: usize,
}
