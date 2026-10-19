#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeLease {
    pub soul: String,
    pub generation: u64,
    pub state: String,
    pub remaining: u8,
    pub cadence_millis: u64,
    pub next_millis: Option<i64>,
    pub last_wake_millis: Option<i64>,
    pub created: String,
    pub updated: String,
}

#[derive(Clone, Copy)]
pub struct WakeOfferDraft<'a> {
    pub soul: &'a str,
    pub generation: u64,
    pub due_millis: i64,
    pub now_millis: i64,
    pub occurred: &'a str,
    pub notice: crate::store::NoticeDraft<'a>,
    pub gate: usize,
}
