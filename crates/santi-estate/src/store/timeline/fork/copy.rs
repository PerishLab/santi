use super::*;

pub(super) async fn entries(
    tx: &mut Tx<'_, Sqlite>,
    parent: i64,
    child: i64,
    at: i64,
) -> Result<(), keel::adapt::Error> {
    let entries = tx
        .ask(
            &form("StrandEntry")
                .when("strand", Op::Eq, &parent.to_string())
                .when("sequence", Op::Le, &at.to_string())
                .order("sequence", Rank::Asc),
        )
        .await?
        .rows()
        .to_vec();
    for entry in entries {
        let kind = text(&entry, "target_type")?;
        let column = crate::store::entry::column(kind)?;
        tx.put(
            "StrandEntry",
            &[
                ("target_type", kind),
                (column, &int(&entry, column)?.to_string()),
                ("sequence", &int(&entry, "sequence")?.to_string()),
                ("created", text(&entry, "created")?),
                ("strand", &child.to_string()),
            ],
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn compacts(
    tx: &mut Tx<'_, Sqlite>,
    parent: i64,
    child: i64,
    at: i64,
) -> Result<(), keel::adapt::Error> {
    let compacts = tx
        .ask(
            &form("Compact")
                .when("strand", Op::Eq, &parent.to_string())
                .order("created", Rank::Asc)
                .order("tag", Rank::Asc),
        )
        .await?
        .rows()
        .to_vec();
    for compact in compacts {
        let first = int(&compact, "first")?;
        let last = int(&compact, "last")?;
        let from = Writer(tx).message_sequence(parent, first).await?;
        let to = Writer(tx).message_sequence(parent, last).await?;
        if from > at || to > at {
            continue;
        }
        let tag = santi_model::tag("cmp");
        let first = first.to_string();
        let last = last.to_string();
        let child = child.to_string();
        let mut fields = vec![
            ("tag", tag.as_str()),
            ("summary", text(&compact, "summary")?),
            ("strand", child.as_str()),
            ("first", first.as_str()),
            ("last", last.as_str()),
        ];
        if let Some(created) = compact.text("created") {
            fields.push(("created", created));
        }
        if let Some(metadata) = compact.text("metadata") {
            fields.push(("metadata", metadata));
        }
        tx.put("Compact", &fields).await?;
    }
    Ok(())
}
