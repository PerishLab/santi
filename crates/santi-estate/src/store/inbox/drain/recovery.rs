use super::*;

#[derive(Default)]
pub(super) struct Recovery {
    pub from: Option<i64>,
    pub receipts: Vec<String>,
    pub refusal: Option<Refusal>,
}

pub(super) async fn collect(
    tx: &mut Tx<'_, Sqlite>,
    strand: &Row,
) -> Result<Recovery, keel::adapt::Error> {
    let rows = tx
        .ask(
            &form("InboxReceipt")
                .when("strand", Op::Eq, &strand.key().to_string())
                .when("state", Op::Eq, "failed"),
        )
        .await?;
    let mut recovered = Recovery::default();
    let seen = strand
        .int("seen")
        .ok_or_else(|| keel::adapt::Error::Adapt("strand seen missing".into()))?;
    for receipt in rows.rows() {
        let last = tx
            .one(
                &form("ReceiptTransition")
                    .when("receipt", Op::Eq, &receipt.key().to_string())
                    .order("sequence", Rank::Desc)
                    .top(1),
            )
            .await?
            .ok_or_else(|| keel::adapt::Error::Adapt("failed receipt transition missing".into()))?;
        let Some(turn) = last.int("turn") else {
            continue;
        };
        let turn = tx
            .one(&form("Turn").when("id", Op::Eq, &turn.to_string()))
            .await?
            .ok_or_else(|| keel::adapt::Error::Adapt("failed receipt turn missing".into()))?;
        if turn.int("strand") != Some(strand.key()) || last.text("state") != Some("failed") {
            return Err(keel::adapt::Error::Adapt(
                "failed receipt identity disagrees".into(),
            ));
        }
        let from = turn
            .int("from")
            .ok_or_else(|| keel::adapt::Error::Adapt("failed turn boundary missing".into()))?;
        if from <= seen {
            continue;
        }
        if let Some(refusal) = effects(tx, receipt).await? {
            recovered.refusal = Some(refusal);
            return Ok(recovered);
        }
        recovered.from = Some(recovered.from.unwrap_or(from).max(from));
        recovered.receipts.push(
            receipt
                .text("tag")
                .ok_or_else(|| keel::adapt::Error::Adapt("failed receipt tag missing".into()))?
                .to_string(),
        );
    }
    Ok(recovered)
}

async fn effects(
    tx: &mut Tx<'_, Sqlite>,
    receipt: &Row,
) -> Result<Option<Refusal>, keel::adapt::Error> {
    let transitions = tx
        .ask(&form("ReceiptTransition").when("receipt", Op::Eq, &receipt.key().to_string()))
        .await?;
    let turns = transitions
        .rows()
        .iter()
        .filter_map(|row| row.int("turn"))
        .collect::<std::collections::BTreeSet<_>>();
    for turn in turns {
        let effects = tx
            .ask(&form("StrandEffect").when("turn", Op::Eq, &turn.to_string()))
            .await?;
        for effect in effects.rows() {
            if matches!(
                effect.text("state"),
                Some("prepared" | "settled_not_applied")
            ) {
                continue;
            }
            let inbox = receipt.text("tag").unwrap_or("missing");
            let tag = effect.text("tag").unwrap_or("missing");
            let state = effect.text("state").unwrap_or("missing");
            return Ok(Some(Refusal {
                inbox: inbox.to_string(),
                effect: tag.to_string(),
                state: state.to_string(),
            }));
        }
    }
    Ok(None)
}

pub(super) async fn bind(
    tx: &mut Tx<'_, Sqlite>,
    receipts: Vec<String>,
    draft: &DrainDraft<'_>,
) -> Result<(), keel::adapt::Error> {
    for inbox in receipts {
        receipt::shift(
            tx,
            super::super::ReceiptDraft {
                inbox: &inbox,
                state: receipt_model::State::Driving,
                turn: Some(draft.turn),
                incident: None,
                rebuilt: None,
                occurred: draft.created,
            },
        )
        .await?;
    }
    Ok(())
}
