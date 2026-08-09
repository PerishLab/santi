use super::super::keys::Stroke;
use super::{Beat, State, Step};

impl State {
    pub(crate) fn struck(&mut self, stroke: Option<Stroke>) -> Step {
        let Some(stroke) = stroke else {
            return Step::Leave;
        };
        match stroke {
            Stroke::Quit => Step::Leave,
            Stroke::Enter => self.entered(),
            Stroke::Newline => {
                self.insert("\n");
                Step::Stay
            }
            Stroke::Paste(text) => {
                self.insert(&text);
                Step::Stay
            }
            Stroke::Erase => {
                self.erase();
                Step::Stay
            }
            Stroke::Delete => {
                self.delete();
                Step::Stay
            }
            Stroke::Left => {
                self.left();
                Step::Stay
            }
            Stroke::Right => {
                self.right();
                Step::Stay
            }
            Stroke::WordLeft => {
                self.leftward();
                Step::Stay
            }
            Stroke::WordRight => {
                self.rightward();
                Step::Stay
            }
            Stroke::Home => {
                self.home();
                Step::Stay
            }
            Stroke::End => {
                self.end();
                Step::Stay
            }
            Stroke::Up => {
                self.up();
                Step::Stay
            }
            Stroke::Down => {
                self.down();
                Step::Stay
            }
            Stroke::ScrollUp => {
                self.higher(1);
                Step::Stay
            }
            Stroke::ScrollDown => {
                self.lower(1);
                Step::Stay
            }
            Stroke::PageUp => {
                self.higher(self.viewport);
                Step::Stay
            }
            Stroke::PageDown => {
                self.lower(self.viewport);
                Step::Stay
            }
            Stroke::Bottom => {
                self.bottom();
                Step::Stay
            }
            Stroke::Verbose => {
                self.verbose = !self.verbose;
                Step::Stay
            }
            Stroke::Copy => Step::Copy(self.transcript()),
            Stroke::Interrupt if self.selection.is_some() => {
                self.selection = None;
                Step::Stay
            }
            Stroke::Interrupt => match (self.busy, self.turn.clone()) {
                (true, Some(turn)) => {
                    self.push(format!(
                        "interrupt requested for turn {turn}; the accepted message keeps its \
                         receipt and must not be resent"
                    ));
                    Step::Stop(turn)
                }
                (true, None) => {
                    self.push(
                        "interrupt unavailable: no turn has been named on this stream yet"
                            .to_string(),
                    );
                    Step::Stay
                }
                (false, _) => Step::Stay,
            },
            Stroke::Press(x, y) => {
                self.press(x, y);
                Step::Stay
            }
            Stroke::Drag(x, y) => {
                self.drag(x, y);
                Step::Stay
            }
            Stroke::Release => match self.selected() {
                Some(text) => {
                    self.selection = None;
                    Step::Copy(text)
                }
                None => {
                    self.selection = None;
                    Step::Stay
                }
            },
            Stroke::Typed(character) => {
                self.insert(&character.to_string());
                Step::Stay
            }
            Stroke::Redraw => Step::Stay,
        }
    }

    fn entered(&mut self) -> Step {
        if self.typed == "/exit" {
            self.take();
            return Step::Leave;
        }
        if self.typed == "/reload" {
            self.take();
            return self.reload();
        }
        if self.typed == "/status" {
            self.take();
            self.context = "context: refreshing".to_string();
            return Step::Refresh;
        }
        if self.typed.trim().is_empty() {
            return Step::Stay;
        }
        self.spoken()
    }

    fn reload(&mut self) -> Step {
        if self.unsettled {
            self.push(
                "reload refused: the prior send is not durably settled; follow the displayed recovery guidance"
                    .to_string(),
            );
            return Step::Stay;
        }
        if self.busy {
            self.push(
                "reload refused: wait for the active send and its receipt to settle".to_string(),
            );
            return Step::Stay;
        }
        Step::Reload
    }

    fn spoken(&mut self) -> Step {
        if self.unsettled {
            self.push(
                "the prior send is not durably settled; follow the displayed recovery guidance before sending again"
                    .to_string(),
            );
            return Step::Stay;
        }
        if self.busy {
            self.push(
                "a send is already in flight; your draft is preserved until its receipt settles"
                    .to_string(),
            );
            return Step::Stay;
        }
        let text = self.take().expect("checked nonblank draft");
        self.push(format!("you> {text}"));
        self.busy = true;
        Step::Speak(text)
    }

    pub(crate) fn heard(&mut self, beat: Option<Beat>) -> Step {
        let Some(beat) = beat else {
            return Step::Stay;
        };
        let settled = matches!(beat, Beat::Settled(_) | Beat::Broken(_));
        self.absorb(beat);
        if settled { Step::Refresh } else { Step::Stay }
    }
}
