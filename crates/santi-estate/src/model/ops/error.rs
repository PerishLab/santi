use super::super::ledger::{Soul, Strand};
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(crate) struct SoulIncident {
    #[field(string, unique)]
    tag: string,
    #[field(string)]
    code: string,
    #[field(string, unique = soul, opt)]
    active: string,
    #[field(
        string,
        values = [
            "exhausted",
            "internal",
            "invalid",
            "missing",
            "unauthorized",
            "unavailable"
        ]
    )]
    category: string,
    #[field(string, values = ["error"])]
    severity: string,
    #[field(string, values = ["changed", "later", "never", "resolved"])]
    retry: string,
    #[field(string)]
    exposure: string,
    #[field(string)]
    source_component: string,
    #[field(string)]
    source_operation: string,
    #[field(string)]
    latest_source_component: string,
    #[field(string)]
    latest_source_operation: string,
    #[field(string)]
    message: string,
    #[field(string)]
    latest_message: string,
    #[field(string)]
    context: string,
    #[field(string)]
    latest_context: string,
    #[field(int, min = 1)]
    occurrence_count: int,
    #[field(int, min = 1)]
    revision: int,
    #[field(string)]
    first_seen: string,
    #[field(string)]
    last_seen: string,
    #[field(string, opt)]
    resolved: string,
    #[field(string, opt)]
    resolved_by: string,
    #[field(string, unique)]
    opened_event: string,
    #[field(string)]
    opened_payload: string,
    #[field(string, opt)]
    opened_delivered: string,
    #[field(string, unique, opt)]
    resolved_event: string,
    #[field(string, opt)]
    resolved_payload: string,
    #[field(string, opt)]
    resolved_delivered: string,
    #[relation(Soul, many2one, root)]
    soul: Soul,
}

#[resource]
pub(crate) struct StrandIncident {
    #[field(string, unique)]
    tag: string,
    #[field(string)]
    code: string,
    #[field(string, unique = strand, opt)]
    active: string,
    #[field(
        string,
        values = [
            "exhausted",
            "internal",
            "invalid",
            "missing",
            "unauthorized",
            "unavailable"
        ]
    )]
    category: string,
    #[field(string, values = ["error"])]
    severity: string,
    #[field(string, values = ["changed", "later", "never", "resolved"])]
    retry: string,
    #[field(string)]
    exposure: string,
    #[field(string)]
    source_component: string,
    #[field(string)]
    source_operation: string,
    #[field(string)]
    latest_source_component: string,
    #[field(string)]
    latest_source_operation: string,
    #[field(string)]
    message: string,
    #[field(string)]
    latest_message: string,
    #[field(string)]
    context: string,
    #[field(string)]
    latest_context: string,
    #[field(int, min = 1)]
    occurrence_count: int,
    #[field(int, min = 1)]
    revision: int,
    #[field(string)]
    first_seen: string,
    #[field(string)]
    last_seen: string,
    #[field(string, opt)]
    resolved: string,
    #[field(string, opt)]
    resolved_by: string,
    #[field(string, unique)]
    opened_event: string,
    #[field(string)]
    opened_payload: string,
    #[field(string, opt)]
    opened_delivered: string,
    #[field(string, unique, opt)]
    resolved_event: string,
    #[field(string, opt)]
    resolved_payload: string,
    #[field(string, opt)]
    resolved_delivered: string,
    #[relation(Strand, many2one, root)]
    strand: Strand,
}
