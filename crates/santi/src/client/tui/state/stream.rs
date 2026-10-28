use super::super::parse::{Spoken, now};
use super::{Beat, Entry, Kind, State};

impl State {
    pub(in crate::client::tui) fn absorb(&mut self, beat: Beat) {
        match beat {
            Beat::Open(who) => {
                self.entries.push(Entry::Said(Spoken {
                    seq: 0,
                    stamp: now(),
                    who,
                    lines: vec![String::new()],
                }));
                self.changed();
            }
            Beat::Speech(text) => self.append(&text),
            Beat::Event(kind, turn, line) => {
                if let Some(held) = turn.clone() {
                    self.turn = Some(held);
                }
                self.activity(kind, turn, line);
            }
            Beat::Lost(detail) => {
                self.deaf = true;
                self.push(format!("stream lost: {detail}; reconnecting"));
            }
            Beat::Live => {
                if self.deaf {
                    self.deaf = false;
                    self.push(
                        "stream resumed; events during the outage are not recoverable from the \
                         server and are not replayed"
                            .to_string(),
                    );
                }
            }
            Beat::Accepted(receipt) => self.receipt = Some(receipt),
            Beat::Settled(receipt) => {
                self.finish();
                self.push(format!(
                    "send completed: receipt {receipt} is durably completed; do not resend"
                ));
            }
            Beat::Failed(receipt, turn, detail) => self.failed(receipt, turn, detail),
            Beat::Broken(detail) => {
                self.finish();
                self.push(detail);
            }
            Beat::Unsettled(detail) => {
                self.busy = true;
                self.unsettled = true;
                self.push(detail);
            }
        }
    }

    fn finish(&mut self) {
        self.busy = false;
        self.unsettled = false;
        self.receipt = None;
        self.since = None;
        self.stop = None;
    }

    fn failed(&mut self, receipt: String, turn: Option<String>, detail: String) {
        if self.receipt.as_deref().is_some_and(|held| held != receipt) {
            self.detail(format!(
                "ignored outcome for another receipt {receipt}: {detail}"
            ));
            return;
        }
        let interrupted = turn.as_ref().is_some_and(|turn| self.entries.iter().any(|entry| {
            matches!(entry, Entry::Activity {turn: Some(held), items} if held == turn && items.iter().any(|(kind, _)| *kind == Kind::Interrupted))
        }));
        self.finish();
        self.detail(format!(
            "receipt {receipt}; turn {}: {detail}",
            turn.as_deref().unwrap_or("unknown")
        ));
        self.push(if interrupted {
            "Interrupted. Send a new instruction to continue; prior effects are retained.".into()
        } else {
            "Task failed. Inspect details before continuing; prior effects may remain.".into()
        });
    }

    pub(in crate::client::tui) fn stopped(&mut self, turn: String, result: Result<bool, String>) {
        if !self.busy || self.stop.as_ref().is_none_or(|(held, _)| held != &turn) {
            return;
        }
        match result {
            Ok(true) => {
                self.stop = Some((turn.clone(), true));
                self.detail(format!(
                    "interrupt accepted for turn {turn}; awaiting its durable outcome"
                ));
            }
            Ok(false) => {
                self.stop = None;
                self.push("Interruption was not accepted; awaiting the task's outcome.".into());
            }
            Err(error) => {
                self.push(
                    "Interruption is unconfirmed; the task may still be running. See details."
                        .into(),
                );
                self.detail(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::keys::Stroke;
    use super::super::tests::{drawn, enter, fresh};
    use super::super::{Beat, Entry, Kind, Step};
    #[test]
    fn collapse() {
        let mut state = fresh();
        state.absorb(Beat::Speech("soul> hello".into()));
        state.absorb(Beat::Event(
            Kind::Thinking,
            Some("turn_8f8b874793".into()),
            "thinking started t1".into(),
        ));
        state.absorb(Beat::Event(
            Kind::Tool,
            Some("turn_8f8b874793".into()),
            "tool call read_file".into(),
        ));
        state.absorb(Beat::Event(
            Kind::Tool,
            Some("turn_8f8b874793".into()),
            "tool result ok".into(),
        ));

        let Some(Entry::Activity { items, .. }) = state.entries.last() else {
            panic!("consecutive protocol events belong to one group");
        };
        assert_eq!(items.len(), 3, "the events are kept, only quieted");
        assert_eq!(
            Entry::summary(Some("turn_8f8b874793"), items),
            "▸ turn 8f8b8747  thinking 1  tool 2",
            "the collapsed line says what happened without replaying it"
        );
        assert!(!state.verbose, "the screen is a conversation first");

        state.absorb(Beat::Event(
            Kind::Fault,
            Some("turn_8f8b874793".into()),
            "turn failed: boom".into(),
        ));
        assert!(
            matches!(state.entries.last(), Some(Entry::Notice(line)) if line.contains("boom")),
            "a fault must never be collapsible out of sight"
        );

        let text = state.transcript();
        assert!(text.contains("soul> hello"));
        assert!(text.contains("tool call read_file"));
        assert!(text.contains("turn failed: boom"));
    }

    #[test]
    fn verbosity() {
        let mut state = fresh();
        state.absorb(Beat::Event(
            Kind::Thinking,
            Some("turn_8f8b874793".into()),
            "thinking started t1".into(),
        ));
        let before = state.transcript();

        state.struck(Some(Stroke::Verbose));
        assert!(state.verbose);
        state.struck(Some(Stroke::Verbose));
        assert!(!state.verbose);

        assert_eq!(
            state.transcript(),
            before,
            "toggling how much is shown must not change what is held"
        );
    }

    #[test]
    fn lost() {
        let mut state = fresh();
        state.absorb(Beat::Live);
        assert!(!state.deaf);
        assert!(
            !state.top().contains("not hearing"),
            "a healthy stream says nothing about itself"
        );

        state.absorb(Beat::Lost("connection reset".into()));
        assert!(state.deaf);
        assert!(
            state.top().contains("not hearing"),
            "a deaf shell must not read as a quiet strand"
        );
        assert!(state.transcript().contains("connection reset"));

        state.absorb(Beat::Live);
        assert!(!state.deaf);
        assert!(
            state.transcript().contains("not recoverable"),
            "reconnection must admit that the gap cannot be recovered, not imply it resumed"
        );
    }

    #[test]
    fn interrupt() {
        let mut state = fresh();
        assert!(matches!(enter(&mut state, "a message"), Step::Speak(_)));
        state.absorb(Beat::Event(
            Kind::Turn,
            Some("turn_8f8b87".into()),
            "turn started turn_8f8b87".into(),
        ));

        assert_eq!(
            state.struck(Some(Stroke::Interrupt)),
            Step::Stop("turn_8f8b87".to_string())
        );
        assert!(
            state.busy,
            "requesting a stop settles nothing; only a durable outcome may clear the send"
        );
        assert!(state.transcript().contains("must not be resent"));
    }

    #[test]
    fn unnamed() {
        let mut state = fresh();
        assert!(matches!(enter(&mut state, "a message"), Step::Speak(_)));

        assert_eq!(state.struck(Some(Stroke::Interrupt)), Step::Stay);
        assert!(state.transcript().contains("no turn has been named"));
    }

    #[test]
    fn escape() {
        let mut state = drawn(&["alpha", "beta"]);
        assert!(matches!(enter(&mut state, "a message"), Step::Speak(_)));
        state.absorb(Beat::Event(
            Kind::Turn,
            Some("turn_8f8b87".into()),
            "turn started".into(),
        ));
        state.struck(Some(Stroke::Press(0, 0)));
        state.struck(Some(Stroke::Drag(4, 0)));

        assert_eq!(
            state.struck(Some(Stroke::Interrupt)),
            Step::Stay,
            "the nearer meaning wins: dismiss the highlight, do not stop the turn"
        );
        assert!(state.highlight(0).is_none());
        assert_eq!(
            state.struck(Some(Stroke::Interrupt)),
            Step::Stop("turn_8f8b87".to_string()),
            "a second escape, with nothing selected, is the interrupt"
        );
    }

    #[test]
    fn regrouped() {
        let mut state = fresh();
        state.absorb(Beat::Event(
            Kind::Tool,
            Some("turn_aaaa1111".into()),
            "tool call one".into(),
        ));
        state.absorb(Beat::Event(Kind::Tool, None, "tool result one".into()));
        state.absorb(Beat::Event(
            Kind::Tool,
            Some("turn_bbbb2222".into()),
            "tool call two".into(),
        ));

        let groups = state
            .entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Activity { .. }))
            .count();
        assert_eq!(groups, 2, "a new turn opens a new group");

        let Some(Entry::Activity { turn, items }) = state.entries.last() else {
            panic!("the last group belongs to the second turn");
        };
        assert_eq!(turn.as_deref(), Some("turn_bbbb2222"));
        assert_eq!(items.len(), 1);
    }
}
