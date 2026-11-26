use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn lease() -> Spec {
    Spec::build("WakeLease")
        .field("generation", Atom::Int)
        .rule("generation", Rule::new().min(1))
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().values(&["active", "expired", "revoked", "silent"]),
        )
        .field("remaining", Atom::Int)
        .rule("remaining", Rule::new().min(0))
        .field("cadence_millis", Atom::Int)
        .rule("cadence_millis", Rule::new().min(1))
        .optional("next_millis", Atom::Int, Only::Free)
        .rule("next_millis", Rule::new().min(1))
        .optional("last_wake_millis", Atom::Int, Only::Free)
        .rule("last_wake_millis", Rule::new().min(1))
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("soul", Bond::One2one, "Soul")
        .seal()
}
