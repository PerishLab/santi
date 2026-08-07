use super::REFERENCE;

const BANDS: [(usize, &str); 3] = [(50, "soft"), (75, "firm"), (90, "hard")];

pub(super) fn banded(budget: Option<usize>, total: usize) -> &'static str {
    let Some(budget) = budget.filter(|budget| *budget > 0) else {
        return "soft";
    };
    let share = total.saturating_mul(100) / budget;
    BANDS
        .iter()
        .rev()
        .find(|(edge, _)| share >= *edge)
        .map_or("under", |(_, name)| *name)
}

pub(super) fn floor(budget: Option<usize>, total: usize) -> usize {
    let Some(budget) = budget.filter(|budget| *budget > 0) else {
        return REFERENCE;
    };
    let share = total.saturating_mul(100) / budget;
    BANDS
        .iter()
        .rev()
        .find(|(edge, _)| share >= *edge)
        .map_or(REFERENCE, |(edge, _)| budget * edge / 100)
}
