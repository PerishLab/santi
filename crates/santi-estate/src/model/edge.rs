use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(webhook())
        .add(delivery())
        .add(acknowledgement())
        .add(window())
        .add(downstream())
        .add(ingest())
        .add(environ("SoulEnviron", "soul", "Soul"))
        .add(environ("StrandEnviron", "strand", "Strand"));
}

fn webhook() -> Spec {
    Spec::build("Webhook")
        .sole("name", Atom::Text)
        .field("adaptor", Atom::Text)
        .field("strategy", Atom::Text)
        .field("credential", Atom::Text)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("soul", Bond::Many2one, "Soul")
        .seal()
}

fn delivery() -> Spec {
    Spec::build("WebhookDelivery")
        .per("delivery", Atom::Text, &["webhook"])
        .field("request_sha256", Atom::Text)
        .field("created", Atom::Text)
        .root("receipt", Bond::One2one, "InboxReceipt")
        .bond("webhook", Bond::Many2one, "Webhook", &[])
        .freeze()
        .seal()
}

fn downstream() -> Spec {
    Spec::build("Downstream")
        .sole("tag", Atom::Text)
        .sole("prefix", Atom::Text)
        .sole("credential_sha256", Atom::Text)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .seal()
}

fn ingest() -> Spec {
    Spec::build("DownstreamIngest")
        .per("request", Atom::Text, &["downstream"])
        .field("request_sha256", Atom::Text)
        .field("created", Atom::Text)
        .root("receipt", Bond::One2one, "InboxReceipt")
        .bond("downstream", Bond::Many2one, "Downstream", &[])
        .freeze()
        .seal()
}

fn environ(name: &str, scope: &str, target: &str) -> Spec {
    Spec::build(name)
        .per("name", Atom::Text, &[scope])
        .field("value", Atom::Text)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root(scope, Bond::Many2one, target)
        .seal()
}

fn acknowledgement() -> Spec {
    Spec::build("Acknowledgement")
        .field("intent", Atom::Text)
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().values(&[
                "dispatching",
                "accepted",
                "rejected",
                "unsent",
                "unknown",
                "observed",
                "not_applied",
            ]),
        )
        .optional("result", Atom::Text, Only::Free)
        .optional("resolution", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("receipt", Bond::One2one, "InboxReceipt")
        .seal()
}

fn window() -> Spec {
    Spec::build("Window")
        .free("input", Bond::One2one, "Message")
        .field("record", Atom::Text)
        .root("receipt", Bond::One2one, "InboxReceipt")
        .seal()
}
