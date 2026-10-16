use crate::{Fault, turn::Turn};

pub(crate) enum Outcome {
    Started(Turn),
    Running(Turn),
    Idle,
    Held(Fault),
    Paused,
    Failed(Fault),
}
