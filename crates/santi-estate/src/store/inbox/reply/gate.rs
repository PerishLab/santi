use super::{Change, Error, Sqlite};
use keel::{Op, Rank, form};
use santi_model::receipt::reply::{Owner, State, Window};

pub(super) async fn check(
    tx: &mut keel::Tx<'_, Sqlite>,
    receipt: i64,
    window: &mut Window,
    change: &Change<'_>,
) -> Result<(), Error> {
    if !matches!(change, Change::Dispatch) {
        return Ok(());
    }
    let latest = tx
        .one(
            &form("ReceiptTransition")
                .when("receipt", Op::Eq, &receipt.to_string())
                .order("sequence", Rank::Desc)
                .top(1),
        )
        .await?
        .ok_or_else(|| Error::Missing("receipt transition".into()))?;
    let sequence = latest
        .int("sequence")
        .ok_or_else(|| Error::Missing("receipt sequence".into()))?;
    let operation = format!("runtime:{sequence}");
    for attempt in &mut window.attempts {
        if attempt.owner != Owner::Runtime || attempt.state != State::Queued {
            continue;
        }
        let stale = attempt.operation != operation;
        let completed = latest.text("state") == Some("completed") && window.message.is_none();
        if stale || completed {
            attempt.state = State::Cancelled;
        }
    }
    Ok(())
}
