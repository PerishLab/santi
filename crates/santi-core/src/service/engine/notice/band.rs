use super::REFERENCE;
use crate::service::tools::Gauge;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Band {
    Under,
    Soft,
    Firm,
    Hard,
    Slots,
}

impl Band {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Under => "under",
            Self::Soft => "soft",
            Self::Firm => "firm",
            Self::Hard => "hard",
            Self::Slots => "slots",
        }
    }
}

const BANDS: [(i64, Band); 3] = [(50, Band::Soft), (75, Band::Firm), (90, Band::Hard)];

pub(super) fn refused(gauge: &Gauge) -> bool {
    gauge.ceiling.is_some_and(|ceiling| {
        gauge.active > ceiling || gauge.settled > gauge.allowance || gauge.held > gauge.slots
    })
}

pub(super) fn banded(gauge: &Gauge) -> Band {
    if gauge.held > gauge.slots {
        return Band::Slots;
    }
    let Some(ceiling) = gauge.ceiling.filter(|ceiling| *ceiling > 0) else {
        return if gauge.active >= REFERENCE as i64 {
            Band::Soft
        } else {
            Band::Under
        };
    };
    let share = gauge.active.saturating_mul(100) / ceiling;
    BANDS
        .iter()
        .rev()
        .find(|(edge, _)| share >= *edge)
        .map_or(Band::Under, |(_, band)| *band)
}
