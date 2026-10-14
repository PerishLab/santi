use crate::service::Service;
use crate::service::flow::failure::{Failure, Operation, Persistence};
use crate::{message, stream};

pub(super) struct Finish {
    pub(super) last: Option<message::Placed>,
    pub(super) response: Option<String>,
    pub(super) settled: bool,
    pub(super) pause: Option<message::Content>,
}

impl Service {
    pub(super) async fn land(&self, strand: &str, turn: &str, finish: Finish) {
        let Finish {
            last,
            response,
            settled: _,
            pause,
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
        let completed = if let Some(content) = pause.as_ref() {
            let source = crate::ingest::Source::new("soul_memory_pause").with_ref(turn.to_string());
            self.store
                .handoff(
                    draft,
                    santi_estate::InboxDraft {
                        tag: &crate::tag("inbox"),
                        strand,
                        kind: message::Kind::SantiSystem,
                        content,
                        source: Some(&source),
                        created: &crate::now(),
                    },
                )
                .await
        } else {
            self.store.finish_turn(draft).await
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
                        text,
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
