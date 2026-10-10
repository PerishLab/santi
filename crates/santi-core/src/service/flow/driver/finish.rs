use crate::service::Service;
use crate::service::flow::failure::{Failure, Operation, Persistence};
use crate::{message, stream};

pub(super) struct Finish {
    pub(super) last: Option<message::Placed>,
    pub(super) response: Option<String>,
    pub(super) handoff: Option<String>,
}

impl Service {
    pub(super) async fn land(&self, strand: &str, turn: &str, finish: Finish) {
        let Finish {
            last,
            response,
            handoff,
        } = finish;
        if let Some(message) = last.as_ref() {
            self.publish(
                strand,
                stream::Payload::Message(crate::message::Beat::Completed {
                    turn: turn.to_string(),
                    message: message.clone(),
                }),
            );
        }
        let metadata = self.provider.metadata();
        let draft = santi_estate::CompletionDraft {
            turn,
            reply: last.as_ref().map(|message| message.message.id.as_str()),
            provider: &metadata.provider,
            model: &metadata.model,
            response: response.as_deref(),
            occurred: &crate::now(),
        };
        let pause = handoff.clone();
        let completed = match handoff {
            Some(detail) => {
                let content = message::Content::text(detail);
                let source =
                    crate::ingest::Source::new("execution_pause").with_ref(turn.to_string());
                self.store
                    .handoff(
                        draft,
                        santi_estate::InboxDraft {
                            tag: &crate::tag("inbox"),
                            strand,
                            kind: message::Kind::SantiSystem,
                            content: &content,
                            source: Some(&source),
                            created: &crate::now(),
                        },
                    )
                    .await
            }
            None => self.store.finish_turn(draft).await,
        };
        match completed {
            Ok(completion) => {
                self.dispatched().await;
                let turned = completion.event;
                let (label, text) = match turned {
                    Some(event) => (Some(event.label), Some(event.text)),
                    None => (None, None),
                };
                self.publish(
                    strand,
                    stream::Payload::Turn(crate::turn::Beat::Completed {
                        turn: turn.to_string(),
                        label,
                        text: pause.or(text),
                    }),
                );
            }
            Err(error) => match self.store.stop(turn).await {
                Ok(Some(stop)) if stop.cause.is_some() => {
                    self.bury(strand, turn, Failure::stopped(stop.cause.unwrap(), ""))
                        .await
                }
                Ok(_) | Err(_) => {
                    self.bury(
                        strand,
                        turn,
                        Failure::runtime(
                            Operation::Persistence(Persistence::Completion),
                            error,
                            "",
                        ),
                    )
                    .await
                }
            },
        }
    }
}
