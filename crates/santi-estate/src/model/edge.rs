use super::inbox::InboxReceipt;
use super::ledger::{Soul, Strand};
use keel::atom::string;
use keel::resource;

#[resource]
pub(crate) struct Webhook {
    #[field(string, unique)]
    name: string,
    #[field(string)]
    adaptor: string,
    #[field(string)]
    strategy: string,
    #[field(string)]
    credential: string,
    #[field(string)]
    created: string,
    #[field(string)]
    updated: string,
    #[relation(Soul, many2one, root)]
    soul: Soul,
}

#[resource(frozen)]
pub(crate) struct WebhookDelivery {
    #[field(string, unique = webhook)]
    delivery: string,
    #[field(string)]
    request_sha256: string,
    #[field(string)]
    created: string,
    #[relation(InboxReceipt, one2one, root)]
    receipt: InboxReceipt,
    #[relation(Webhook, many2one)]
    webhook: Webhook,
}

#[resource]
pub(crate) struct Downstream {
    #[field(string, unique)]
    tag: string,
    #[field(string, unique)]
    prefix: string,
    #[field(string, unique)]
    credential_sha256: string,
    #[field(string)]
    created: string,
    #[field(string)]
    updated: string,
}

#[resource(frozen)]
pub(crate) struct DownstreamIngest {
    #[field(string, unique = downstream)]
    request: string,
    #[field(string)]
    request_sha256: string,
    #[field(string)]
    created: string,
    #[relation(InboxReceipt, one2one, root)]
    receipt: InboxReceipt,
    #[relation(Downstream, many2one)]
    downstream: Downstream,
}

#[resource]
pub(crate) struct SoulEnviron {
    #[field(string, unique = soul)]
    name: string,
    #[field(string)]
    value: string,
    #[field(string)]
    created: string,
    #[field(string)]
    updated: string,
    #[relation(Soul, many2one, root)]
    soul: Soul,
}

#[resource]
pub(crate) struct StrandEnviron {
    #[field(string, unique = strand)]
    name: string,
    #[field(string)]
    value: string,
    #[field(string)]
    created: string,
    #[field(string)]
    updated: string,
    #[relation(Strand, many2one, root)]
    strand: Strand,
}
