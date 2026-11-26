use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph
        .add(event())
        .add(result())
        .add(output())
        .add(failure())
        .add(thinking())
        .add(completion())
        .add(failed())
        .add(round())
        .add(compact());
}

fn event() -> Spec {
    Spec::build("MessageEvent")
        .sole("tag", Atom::Text)
        .field("action", Atom::Text)
        .rule(
            "action",
            Rule::new().values(&["delete", "fix", "insert", "patch", "prepared", "remove"]),
        )
        .field("actor_type", Atom::Text)
        .rule("actor_type", Rule::new().values(&["soul", "system"]))
        .field("actor", Atom::Text)
        .field("base_version", Atom::Int)
        .rule("base_version", Rule::new().min(1))
        .field("payload", Atom::Text)
        .field("created", Atom::Text)
        .root("message", Bond::Many2one, "Message")
        .freeze()
        .seal()
}

fn result() -> Spec {
    Spec::build("ToolResult")
        .sole("tag", Atom::Text)
        .field("created", Atom::Text)
        .root("call", Bond::One2one, "ToolCall")
        .freeze()
        .seal()
}

fn output() -> Spec {
    Spec::build("ToolOutput")
        .field("output", Atom::Text)
        .root("result", Bond::One2one, "ToolResult")
        .freeze()
        .seal()
}

fn failure() -> Spec {
    Spec::build("ToolFailure")
        .field("error", Atom::Text)
        .root("result", Bond::One2one, "ToolResult")
        .freeze()
        .seal()
}

fn thinking() -> Spec {
    Spec::build("ThinkingSpan")
        .sole("tag", Atom::Text)
        .optional("response", Atom::Text, Only::Free)
        .optional("summary", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .root("turn", Bond::Many2one, "Turn")
        .seal()
}

fn completion() -> Spec {
    Spec::build("ThinkingCompletion")
        .field("reason", Atom::Text)
        .rule(
            "reason",
            Rule::new().values(&["called", "finished", "spoke"]),
        )
        .field("finished", Atom::Text)
        .root("thinking", Bond::One2one, "ThinkingSpan")
        .freeze()
        .seal()
}

fn failed() -> Spec {
    Spec::build("ThinkingFailure")
        .field("error", Atom::Text)
        .field("finished", Atom::Text)
        .root("thinking", Bond::One2one, "ThinkingSpan")
        .freeze()
        .seal()
}

fn round() -> Spec {
    Spec::build("Round")
        .field("record", Atom::Text)
        .root("thinking", Bond::One2one, "ThinkingSpan")
        .freeze()
        .seal()
}

fn compact() -> Spec {
    Spec::build("Compact")
        .sole("tag", Atom::Text)
        .field("summary", Atom::Text)
        .optional("created", Atom::Text, Only::Free)
        .optional("metadata", Atom::Text, Only::Free)
        .root("strand", Bond::Many2one, "Strand")
        .free("first", Bond::Many2one, "Message")
        .free("last", Bond::Many2one, "Message")
        .free("head", Bond::Many2one, "StrandEntry")
        .free("tail", Bond::Many2one, "StrandEntry")
        .freeze()
        .seal()
}
