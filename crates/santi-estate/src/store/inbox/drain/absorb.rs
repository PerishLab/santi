use super::*;
use santi_model::message;

pub struct Intake {
    strand: String,
    turn: String,
    next: i64,
    created: String,
    pending: Vec<Pending>,
    messages: Vec<message::Placed>,
}

impl Intake {
    pub fn messages(&self) -> &[message::Placed] {
        &self.messages
    }
}

impl Store {
    pub async fn intake(
        &self,
        strand: &str,
        turn: &str,
        created: &str,
    ) -> Result<Option<Intake>, String> {
        self.core
            .batch(async |tx| {
                let row = tx
                    .one(&form("Strand").when("tag", Op::Eq, strand))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(strand.into()))?;
                if running(tx, row.key()).await?.as_deref() != Some(turn)
                    || stopped(tx, turn).await?
                {
                    return Ok(None);
                }
                let pending = pending(tx, row.key()).await?;
                if pending.is_empty() {
                    return Ok(None);
                }
                let next = row
                    .int("next")
                    .ok_or_else(|| keel::adapt::Error::Adapt("strand next missing".into()))?;
                let mut messages = Vec::new();
                for item in pending.iter().filter(|item| item.coalesce_key.is_none()) {
                    messages.push(future(
                        (strand, next + messages.len() as i64),
                        &item.kind,
                        &item.content,
                        created,
                    )?);
                }
                let notices: Vec<_> = pending
                    .iter()
                    .filter(|item| item.coalesce_key.is_some())
                    .cloned()
                    .collect();
                if !notices.is_empty() {
                    messages.push(future(
                        (strand, next + messages.len() as i64),
                        "santi_system",
                        &aggregate(&notices, created)?,
                        created,
                    )?);
                }
                Ok(Some(Intake {
                    strand: strand.into(),
                    turn: turn.into(),
                    next,
                    created: created.into(),
                    pending,
                    messages,
                }))
            })
            .await
            .map_err(read::error)
    }

    pub async fn absorb(&self, intake: &Intake) -> Result<bool, String> {
        self.core
            .batch(async |tx| {
                let strand = tx
                    .one(&form("Strand").when("tag", Op::Eq, &intake.strand))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(intake.strand.clone()))?;
                if strand.int("next") != Some(intake.next)
                    || running(tx, strand.key()).await?.as_deref() != Some(intake.turn.as_str())
                    || stopped(tx, &intake.turn).await?
                {
                    return Ok(false);
                }
                let current = pending(tx, strand.key()).await?;
                if intake.pending.iter().any(|item| !current.contains(item)) {
                    return Ok(false);
                }
                let draft = DrainDraft {
                    turn: &intake.turn,
                    strand: &intake.strand,
                    trigger: santi_model::turn::Trigger::StrandSend,
                    actor: "inbox",
                    created: &intake.created,
                };
                let mut writer = Writer(tx);
                let mut written = Vec::new();
                for message in &intake.messages {
                    let content = serde_json::to_string(&message.message.content)
                        .map_err(|e| keel::adapt::Error::Adapt(e.to_string()))?;
                    let kind = match message.message.kind {
                        message::Kind::Text => "text",
                        message::Kind::SantiSystem => "santi_system",
                    };
                    written.push(
                        writer
                            .named(
                                Message {
                                    strand: &strand,
                                    kind,
                                    content: &content,
                                    draft: &draft,
                                },
                                &message.message.id,
                            )
                            .await?,
                    );
                }
                let mut regular = 0;
                for pending in &intake.pending {
                    let index = if pending.coalesce_key.is_some() {
                        written.len() - 1
                    } else {
                        let index = regular;
                        regular += 1;
                        index
                    };
                    writer
                        .consume(
                            Assigned {
                                pending: pending.clone(),
                                message: written[index].clone(),
                            },
                            &draft,
                        )
                        .await?;
                }
                Ok(true)
            })
            .await
            .map_err(read::error)
    }

    pub async fn prepared(
        &self,
        turn: &str,
        round: usize,
        messages: &[String],
        created: &str,
    ) -> Result<(), String> {
        if messages.is_empty() {
            return Ok(());
        }
        self.core
            .batch(async |tx| {
                let held = tx
                    .one(&form("Turn").when("tag", Op::Eq, turn))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(turn.into()))?;
                for message in messages {
                    let row = tx
                        .one(&form("Message").when("tag", Op::Eq, message))
                        .await?
                        .ok_or_else(|| keel::adapt::Error::Missing(message.clone()))?;
                    if row.int("strand") != held.int("strand") {
                        return Err(keel::adapt::Error::Adapt(
                            "prepared input belongs to another strand".into(),
                        ));
                    }
                    tx.put(
                        "MessageEvent",
                        &[
                            ("tag", &santi_model::tag("mev")),
                            ("action", "prepared"),
                            ("actor_type", "system"),
                            ("actor", "driver"),
                            ("base_version", "1"),
                            (
                                "payload",
                                &json!({"kind":"input_prepared", "turn":turn, "round":round})
                                    .to_string(),
                            ),
                            ("created", created),
                            ("message", &row.key().to_string()),
                        ],
                    )
                    .await?;
                }
                Ok(())
            })
            .await
            .map_err(read::error)
    }
}

fn future(
    position: (&str, i64),
    kind: &str,
    content: &str,
    created: &str,
) -> Result<message::Placed, keel::adapt::Error> {
    let content: message::Content =
        serde_json::from_str(content).map_err(|e| keel::adapt::Error::Adapt(e.to_string()))?;
    let id = santi_model::tag("msg");
    Ok(message::Placed {
        relation: message::Relation {
            strand: position.0.into(),
            message: id.clone(),
            seq: position.1,
            created: created.into(),
        },
        text: content.rendered(),
        message: message::Message {
            id,
            role: message::Role::System,
            actor: "inbox".into(),
            kind: match kind {
                "text" => message::Kind::Text,
                "santi_system" => message::Kind::SantiSystem,
                _ => return Err(keel::adapt::Error::Adapt("invalid inbox kind".into())),
            },
            content,
            state: message::State::Fixed,
            version: 1,
            deleted: None,
            created: created.into(),
            updated: created.into(),
        },
    })
}

async fn stopped(tx: &mut Tx<'_, Sqlite>, turn: &str) -> Result<bool, keel::adapt::Error> {
    let held = tx
        .one(&form("Turn").when("tag", Op::Eq, turn))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing(turn.into()))?;
    Ok(tx
        .one(&form("TurnStop").when("turn", Op::Eq, &held.key().to_string()))
        .await?
        .is_some())
}
