use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(inbox())
        .add(slot())
        .add(receipt())
        .add(transition());
}

fn inbox() -> Spec {
    Spec::build("StrandInbox")
        .sole("tag", Atom::Text)
        .field("kind", Atom::Text)
        .rule("kind", Rule::new().values(&["santi_system", "text"]))
        .field("content", Atom::Text)
        .optional("source_type", Atom::Text, Only::Free)
        .optional("source_ref", Atom::Text, Only::Free)
        .optional("source_metadata", Atom::Text, Only::Free)
        .optional(
            "coalesce_key",
            Atom::Text,
            Only::Per(vec!["strand".to_string()]),
        )
        .optional("coalesce_revision", Atom::Int, Only::Free)
        .rule("coalesce_revision", Rule::new().min(1))
        .optional("coalesce_causes", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}

fn slot() -> Spec {
    Spec::build("InboxSlot")
        .per("key", Atom::Text, &["strand"])
        .field("revision", Atom::Int)
        .rule("revision", Rule::new().min(1))
        .field("digest", Atom::Text)
        .field("updated", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .free("inbox", Bond::One2one, "StrandInbox")
        .seal()
}

fn receipt() -> Spec {
    Spec::build("InboxReceipt")
        .sole("tag", Atom::Text)
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().default("accepted").values(&[
                "accepted",
                "completed",
                "driving",
                "failed",
                "recovered",
            ]),
        )
        .field("accepted", Atom::Text)
        .field("updated", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}

fn transition() -> Spec {
    Spec::build("ReceiptTransition")
        .sole("tag", Atom::Text)
        .per("sequence", Atom::Int, &["receipt"])
        .rule("sequence", Rule::new().min(1))
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().values(&["accepted", "completed", "driving", "failed", "recovered"]),
        )
        .field("occurred", Atom::Text)
        .root("receipt", Bond::Many2one, "InboxReceipt")
        .free("turn", Bond::Many2one, "Turn")
        .free("incident", Bond::Many2one, "StrandIncident")
        .freeze()
        .seal()
}
