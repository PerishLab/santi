use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph.add(alarm()).add(delivery());
}

fn alarm() -> Spec {
    Spec::build("Alarm")
        .sole("tag", Atom::Text)
        .per("name", Atom::Text, &["strand"])
        .field("text", Atom::Text)
        .field("cron", Atom::Text)
        .field("generation", Atom::Int)
        .rule("generation", Rule::new().min(1))
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}

fn delivery() -> Spec {
    Spec::build("AlarmDelivery")
        .field("generation", Atom::Int)
        .rule("generation", Rule::new().min(1))
        .per("minute", Atom::Int, &["alarm", "generation"])
        .rule("minute", Rule::new().min(0))
        .root("alarm", Bond::Many2one, "Alarm")
        .bond("receipt", Bond::One2one, "InboxReceipt", &[])
        .freeze()
        .seal()
}
