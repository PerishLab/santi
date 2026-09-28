use keel::Row;
use santi_model::job::Origin;

fn read(origin: &Origin<&Row>) -> Origin<Option<i64>> {
    Origin {
        soul: origin.strand.int("soul"),
        strand: origin.turn.int("strand"),
        turn: origin.call.int("turn"),
        call: origin.effect.int("call"),
        effect: origin.effect.int("turn"),
    }
}

fn expected(origin: &Origin<&Row>) -> Origin<Option<i64>> {
    Origin {
        soul: Some(origin.soul.key()),
        strand: Some(origin.strand.key()),
        turn: Some(origin.turn.key()),
        call: Some(origin.call.key()),
        effect: Some(origin.turn.key()),
    }
}

pub(super) fn validate(origin: Origin<&Row>) -> Result<(), keel::adapt::Error> {
    if read(&origin) != expected(&origin) {
        return Err(adapt("job capability origin is inconsistent"));
    }
    Ok(())
}

pub(super) fn signed(value: u64, label: &str) -> Result<String, keel::adapt::Error> {
    i64::try_from(value)
        .map(|value| value.to_string())
        .map_err(|_| adapt(&format!("{label} is out of range")))
}

pub(super) fn key(row: &Row, relation: &str) -> Result<String, keel::adapt::Error> {
    row.int(relation)
        .map(|key| key.to_string())
        .ok_or_else(|| adapt("job capability relation missing"))
}

pub(super) fn tag(row: &Row) -> Result<String, keel::adapt::Error> {
    row.text("tag")
        .map(str::to_string)
        .ok_or_else(|| adapt("job tag missing"))
}

pub(super) fn adapt(message: &str) -> keel::adapt::Error {
    keel::adapt::Error::Adapt(message.to_string())
}
