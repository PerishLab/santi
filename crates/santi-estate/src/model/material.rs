use super::{
    history::ThinkingSpan,
    ledger::{Soul, Strand, Turn},
};
use keel::{
    atom::{int, string},
    resource,
};

#[resource(frozen)]
pub(crate) struct Revision {
    #[field(string, unique)]
    tag: string,
    #[field(string, unique = owner)]
    key: string,
    #[field(string)]
    reference: string,
    #[field(string)]
    digest: string,
    #[field(string)]
    record: string,
    #[field(string)]
    created: string,
    #[relation(Soul, many2one, root)]
    owner: Soul,
}

#[resource(frozen)]
pub(crate) struct Blob {
    #[field(string, unique = revision)]
    name: string,
    #[field(string)]
    mime: string,
    #[field(string)]
    digest: string,
    #[field(string)]
    text: string,
    #[relation(Revision, many2one, root)]
    revision: Revision,
}

#[resource(frozen)]
pub(crate) struct Declaration {
    #[field(string, unique)]
    tag: string,
    #[field(string, values = ["load", "drop"])]
    action: string,
    #[field(string)]
    reference: string,
    #[field(string)]
    digest: string,
    #[field(string)]
    created: string,
    #[relation(Soul, many2one, root)]
    owner: Soul,
    #[relation(Revision, many2one, opt)]
    previous: Revision,
    #[relation(Revision, many2one, opt)]
    current: Revision,
}

#[resource(frozen)]
pub(crate) struct Change {
    #[field(string, unique)]
    tag: string,
    #[field(string, values = ["load", "drop", "fork"])]
    action: string,
    #[field(string)]
    reference: string,
    #[field(string)]
    digest: string,
    #[field(string)]
    created: string,
    #[relation(Strand, many2one, root)]
    owner: Strand,
    #[relation(Revision, many2one, opt)]
    previous: Revision,
    #[relation(Revision, many2one, opt)]
    current: Revision,
}

#[resource]
pub(crate) struct Shared {
    #[field(string, unique)]
    tag: string,
    #[field(string, unique = owner)]
    reference: string,
    #[field(string)]
    provenance: string,
    #[field(string)]
    updated: string,
    #[relation(Soul, many2one, root)]
    owner: Soul,
    #[relation(Revision, many2one)]
    revision: Revision,
    #[relation(Declaration, many2one)]
    change: Declaration,
}

#[resource]
pub(crate) struct Local {
    #[field(string, unique)]
    tag: string,
    #[field(string, unique = owner)]
    reference: string,
    #[field(string)]
    provenance: string,
    #[field(string)]
    updated: string,
    #[relation(Strand, many2one, root)]
    owner: Strand,
    #[relation(Revision, many2one)]
    revision: Revision,
    #[relation(Change, many2one)]
    change: Change,
}

#[resource]
pub(crate) struct Context {
    #[field(string, unique)]
    tag: string,
    #[field(int, unique = turn, min = 1)]
    round: int,
    #[field(string)]
    digest: string,
    #[field(string)]
    rendered: string,
    #[field(string)]
    created: string,
    #[relation(Turn, many2one, root)]
    turn: Turn,
    #[relation(ThinkingSpan, one2one, opt)]
    thinking: ThinkingSpan,
}

#[resource(frozen)]
pub(crate) struct Usage {
    #[field(int, unique = context, min = 0)]
    position: int,
    #[field(string)]
    provenance: string,
    #[field(int, min = 0)]
    bytes: int,
    #[relation(Context, many2one, root)]
    context: Context,
    #[relation(Revision, many2one)]
    revision: Revision,
}
