use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(incident("SoulIncident", "soul", "Soul"))
        .add(incident("StrandIncident", "strand", "Strand"));
}

fn incident(name: &str, scope: &str, target: &str) -> Spec {
    Spec::build(name)
        .sole("tag", Atom::Text)
        .field("code", Atom::Text)
        .optional("active", Atom::Text, Only::Per(vec![scope.to_string()]))
        .field("category", Atom::Text)
        .rule(
            "category",
            Rule::new().values(&[
                "exhausted",
                "internal",
                "invalid",
                "missing",
                "unauthorized",
                "unavailable",
            ]),
        )
        .field("severity", Atom::Text)
        .rule("severity", Rule::new().values(&["error"]))
        .field("retry", Atom::Text)
        .rule(
            "retry",
            Rule::new().values(&["changed", "later", "never", "resolved"]),
        )
        .field("exposure", Atom::Text)
        .field("source_component", Atom::Text)
        .field("source_operation", Atom::Text)
        .field("latest_source_component", Atom::Text)
        .field("latest_source_operation", Atom::Text)
        .field("message", Atom::Text)
        .field("latest_message", Atom::Text)
        .field("context", Atom::Text)
        .field("latest_context", Atom::Text)
        .field("occurrence_count", Atom::Int)
        .rule("occurrence_count", Rule::new().min(1))
        .field("revision", Atom::Int)
        .rule("revision", Rule::new().min(1))
        .field("first_seen", Atom::Text)
        .field("last_seen", Atom::Text)
        .optional("resolved", Atom::Text, Only::Free)
        .optional("resolved_by", Atom::Text, Only::Free)
        .sole("opened_event", Atom::Text)
        .field("opened_payload", Atom::Text)
        .optional("opened_delivered", Atom::Text, Only::Free)
        .optional("resolved_event", Atom::Text, Only::All)
        .optional("resolved_payload", Atom::Text, Only::Free)
        .optional("resolved_delivered", Atom::Text, Only::Free)
        .root(scope, Bond::Many2one, target)
        .seal()
}
