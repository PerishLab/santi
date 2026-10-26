use super::inbox::InboxReceipt;
use super::ledger::Strand;
use keel::atom::{int, string};
use keel::resource;

#[resource]
pub(crate) struct Alarm {
    #[field(string, unique)]
    tag: string,
    #[field(string, unique = strand)]
    name: string,
    #[field(string)]
    text: string,
    #[field(string)]
    cron: string,
    #[field(int, min = 1)]
    generation: int,
    #[relation(Strand, many2one, root)]
    strand: Strand,
}

#[resource(frozen)]
pub(crate) struct AlarmDelivery {
    #[field(int, min = 1)]
    generation: int,
    #[field(int, unique = (alarm, generation), min = 0)]
    minute: int,
    #[relation(Alarm, many2one, root)]
    alarm: Alarm,
    #[relation(InboxReceipt, one2one)]
    receipt: InboxReceipt,
}
