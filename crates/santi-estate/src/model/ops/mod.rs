use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(crate) mod catalog;
pub(crate) mod error;

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(capability())
        .add(trace())
        .add(stream())
        .add(outbox());
    error::plug(graph);
}

fn capability() -> Spec {
    Spec::build("JobCapability")
        .sole("digest", Atom::Text)
        .field("expires", Atom::Int)
        .rule("expires", Rule::new().min(1))
        .optional("request_sha256", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .bond("soul", Bond::Many2one, "Soul", &[])
        .root("strand", Bond::Many2one, "Strand")
        .bond("turn", Bond::Many2one, "Turn", &[])
        .bond("call", Bond::Many2one, "ToolCall", &[])
        .bond("effect", Bond::Many2one, "StrandEffect", &[])
        .free("consumed", Bond::One2one, "Job")
        .seal()
}

fn trace() -> Spec {
    Spec::build("TraceRecord")
        .sole("tag", Atom::Text)
        .field("boot", Atom::Text)
        .field("span", Atom::Int)
        .rule("span", Rule::new().min(1))
        .optional("parent", Atom::Int, Only::Free)
        .rule("parent", Rule::new().min(1))
        .field("name", Atom::Text)
        .field("tags", Atom::Text)
        .field("opened", Atom::Text)
        .field("closed", Atom::Text)
        .freeze()
        .seal()
}

fn stream() -> Spec {
    Spec::build("OutboxStream").sole("tag", Atom::Text).seal()
}

fn outbox() -> Spec {
    Spec::build("TurnOutbox")
        .sole("tag", Atom::Text)
        .serial("sequence", "stream")
        .field("label", Atom::Text)
        .field("payload", Atom::Text)
        .field("created", Atom::Text)
        .root("turn", Bond::One2one, "Turn")
        .bond("stream", Bond::Many2one, "OutboxStream", &[])
        .freeze()
        .seal()
}
