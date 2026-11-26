use keel::Graph;
use keel::atom::Kind as Atom;
use keel::bond::Kind as Bond;
use keel::spec::{Only, Rule, Spec};

pub(super) fn plug(graph: &mut Graph) {
    graph.add(call()).add(effect()).add(job());
}

fn call() -> Spec {
    Spec::build("ToolCall")
        .sole("tag", Atom::Text)
        .field("tool", Atom::Text)
        .field("arguments", Atom::Text)
        .field("created", Atom::Text)
        .root("turn", Bond::Many2one, "Turn")
        .freeze()
        .seal()
}

fn effect() -> Spec {
    Spec::build("StrandEffect")
        .sole("tag", Atom::Text)
        .field("effect_type", Atom::Text)
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().default("prepared").values(&[
                "dispatching",
                "prepared",
                "settled_applied",
                "settled_not_applied",
                "unknown",
            ]),
        )
        .optional("result", Atom::Text, Only::Free)
        .optional("error", Atom::Text, Only::Free)
        .optional("metadata", Atom::Text, Only::Free)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .optional("dispatched", Atom::Text, Only::Free)
        .optional("settled", Atom::Text, Only::Free)
        .root("turn", Bond::Many2one, "Turn")
        .free("call", Bond::One2one, "ToolCall")
        .seal()
}

fn job() -> Spec {
    Spec::build("Job")
        .sole("tag", Atom::Text)
        .field("description", Atom::Text)
        .field("command", Atom::Text)
        .optional("cwd", Atom::Text, Only::Free)
        .field("timeout_seconds", Atom::Int)
        .rule("timeout_seconds", Rule::new().min(1))
        .field("output_limit_bytes", Atom::Int)
        .rule("output_limit_bytes", Rule::new().min(1))
        .optional("remind_every_seconds", Atom::Int, Only::Free)
        .rule("remind_every_seconds", Rule::new().min(1))
        .field("notify_on_terminal", Atom::Bool)
        .rule("notify_on_terminal", Rule::new().default("false"))
        .field("request_sha256", Atom::Text)
        .sole("capability_sha256", Atom::Text)
        .sole("generation", Atom::Text)
        .sole("supervisor_ref", Atom::Text)
        .optional("environment", Atom::Text, Only::Free)
        .field("state", Atom::Text)
        .rule(
            "state",
            Rule::new().default("submitting").values(&[
                "accepted",
                "cancelled",
                "cancelling",
                "failed",
                "running",
                "submitting",
                "succeeded",
                "timed_out",
                "unknown",
            ]),
        )
        .optional("reason", Atom::Text, Only::Free)
        .optional("exit_code", Atom::Int, Only::Free)
        .field("created", Atom::Text)
        .field("updated", Atom::Text)
        .optional("accepted", Atom::Text, Only::Free)
        .optional("started", Atom::Text, Only::Free)
        .optional("started_millis", Atom::Int, Only::Free)
        .rule("started_millis", Rule::new().min(0))
        .optional("finished", Atom::Text, Only::Free)
        .optional("acknowledged", Atom::Text, Only::Free)
        .optional("terminal_notified", Atom::Text, Only::Free)
        .field("attention_revision", Atom::Int)
        .rule("attention_revision", Rule::new().default("0").min(0))
        .optional("runtime_warned", Atom::Text, Only::Free)
        .optional("output_warned", Atom::Text, Only::Free)
        .optional("last_reminded", Atom::Text, Only::Free)
        .optional("next_reminder", Atom::Text, Only::Free)
        .field("reminder_tick", Atom::Int)
        .rule("reminder_tick", Rule::new().default("0").min(0))
        .root("strand", Bond::Many2one, "Strand")
        .bond("turn", Bond::Many2one, "Turn", &[])
        .bond("call", Bond::Many2one, "ToolCall", &[])
        .bond("effect", Bond::Many2one, "StrandEffect", &[])
        .seal()
}
