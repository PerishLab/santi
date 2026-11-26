use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(crate) struct Soul;
pub(crate) struct Strand;

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(soul())
        .add(strand())
        .add(turn())
        .add(completion())
        .add(failure())
        .add(inference())
        .add(route())
        .add(message())
        .add(entry())
        .add(stop())
        .add(object());
}

fn soul() -> Spec {
    Spec::build("Soul")
        .sole("tag", Atom::Text)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .optional("provider", Atom::Text, Only::Free)
        .optional("model", Atom::Text, Only::Free)
        .optional("reasoning_effort", Atom::Text, Only::Free)
        .seal()
}

fn strand() -> Spec {
    Spec::build("Strand")
        .sole("tag", Atom::Text)
        .optional("label", Atom::Text, Only::Per(vec!["soul".to_string()]))
        .field("memory", Atom::Text)
        .rule("memory", Rule::new().default(""))
        .optional("state", Atom::Text, Only::Free)
        .optional("provider", Atom::Text, Only::Free)
        .optional("model", Atom::Text, Only::Free)
        .optional("reasoning_effort", Atom::Text, Only::Free)
        .field("next", Atom::Int)
        .rule("next", Rule::new().default("1").min(1))
        .field("seen", Atom::Int)
        .rule("seen", Rule::new().default("0").min(0))
        .optional("fork", Atom::Int, Only::Free)
        .rule("fork", Rule::new().min(0))
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("soul", Bond::Many2one, "Soul")
        .free("parent", Bond::Many2one, "Strand")
        .seal()
}

fn turn() -> Spec {
    Spec::build("Turn")
        .sole("tag", Atom::Text)
        .field("trigger", Atom::Text)
        .rule("trigger", Rule::new().values(&["strand_send", "system"]))
        .field("from", Atom::Int)
        .rule("from", Rule::new().min(0))
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}

fn inference() -> Spec {
    Spec::build("Inference")
        .field("provider", Atom::Text)
        .field("model", Atom::Text)
        .optional("reasoning_effort", Atom::Text, Only::Free)
        .field("source", Atom::Text)
        .field("created", Atom::Text)
        .root("turn", Bond::One2one, "Turn")
        .freeze()
        .seal()
}

fn route() -> Spec {
    Spec::build("Route")
        .bond("provider", Bond::Many2one, "Provider", &[])
        .field("provider_name", Atom::Text)
        .bond("access", Bond::Many2one, "Access", &[])
        .field("access_name", Atom::Text)
        .free("family", Bond::Many2one, "Family")
        .optional("family_name", Atom::Text, Only::Free)
        .free("scenario", Bond::Many2one, "Scenario")
        .optional("scenario_name", Atom::Text, Only::Free)
        .free("offering", Bond::Many2one, "Offering")
        .free("candidate", Bond::Many2one, "Candidate")
        .field("endpoint", Atom::Text)
        .field("protocol", Atom::Text)
        .field("custody", Atom::Text)
        .field("replay", Atom::Bool)
        .field("identity", Atom::Text)
        .field("policy", Atom::Text)
        .optional("thinking", Atom::Text, Only::Free)
        .root("turn", Bond::One2one, "Turn")
        .freeze()
        .seal()
}

fn completion() -> Spec {
    Spec::build("TurnCompletion")
        .field("to", Atom::Int)
        .rule("to", Rule::new().min(0))
        .field("finished", Atom::Text)
        .root("turn", Bond::One2one, "Turn")
        .freeze()
        .seal()
}

fn failure() -> Spec {
    Spec::build("TurnFailure")
        .field("error", Atom::Text)
        .field("finished", Atom::Text)
        .root("turn", Bond::One2one, "Turn")
        .freeze()
        .seal()
}

fn message() -> Spec {
    Spec::build("Message")
        .sole("tag", Atom::Text)
        .field("actor_type", Atom::Text)
        .rule("actor_type", Rule::new().values(&["soul", "system"]))
        .field("actor", Atom::Text)
        .field("kind", Atom::Text)
        .rule(
            "kind",
            Rule::new()
                .default("text")
                .values(&["santi_system", "text"]),
        )
        .field("content", Atom::Text)
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().values(&["aborted", "fixed", "pending"]),
        )
        .field("version", Atom::Int)
        .rule("version", Rule::new().default("1").min(1))
        .field("request", Atom::Bool)
        .rule("request", Rule::new().default("false"))
        .optional("deleted", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}

fn entry() -> Spec {
    Spec::build("StrandEntry")
        .field("target_type", Atom::Text)
        .rule(
            "target_type",
            Rule::new().values(&["message", "thinking", "tool_call", "tool_result"]),
        )
        .per("sequence", Atom::Int, &["strand"])
        .rule("sequence", Rule::new().min(1))
        .field("created", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .free("message", Bond::Many2one, "Message")
        .free("thinking", Bond::Many2one, "ThinkingSpan")
        .free("call", Bond::Many2one, "ToolCall")
        .free("result", Bond::Many2one, "ToolResult")
        .freeze()
        .seal()
}

fn stop() -> Spec {
    Spec::build("TurnStop")
        .field("cause", Atom::Text)
        .rule("cause", Rule::new().values(&["operator", "shutdown"]))
        .field("requested", Atom::Text)
        .optional("settled", Atom::Text, Only::Free)
        .root("turn", Bond::One2one, "Turn")
        .seal()
}

fn object() -> Spec {
    Spec::build("Object")
        .per("key", Atom::Text, &["strand"])
        .field("length", Atom::Int)
        .rule("length", Rule::new().min(0))
        .field("sha256", Atom::Text)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("strand", Bond::Many2one, "Strand")
        .seal()
}
