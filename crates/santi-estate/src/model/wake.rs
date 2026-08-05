use super::ledger::Soul;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(crate) struct WakeLease {
    #[field(int, min = 1)]
    generation: int,
    #[field(string, values = ("active", "expired", "revoked", "silent"))]
    state: string,
    #[field(int, min = 0)]
    remaining: int,
    #[field(int, min = 1)]
    cadence_millis: int,
    #[field(int, opt, min = 1)]
    next_millis: int,
    #[field(int, opt, min = 1)]
    last_wake_millis: int,
    #[field(string)]
    created: string,
    #[field(string)]
    updated: string,
    #[relation(Soul, one2one, root)]
    soul: Soul,
}
