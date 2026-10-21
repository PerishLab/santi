use crate::{Fault, engine, strand::Strand};

use super::super::{Service, drive};
use crate::{ingest, message, strand};

pub(in crate::service) struct Ingest<'a> {
    pub(in crate::service) content: message::Content,
    pub(in crate::service) kind: message::Kind,
    pub(in crate::service) trigger: &'a str,
    pub(in crate::service) source: Option<ingest::Source>,
    pub(in crate::service) replay: Option<santi_estate::ReplayDraft<'a>>,
}

pub(in crate::service) struct External<'a> {
    pub(in crate::service) input: crate::service::Envelope<'a>,
    pub(in crate::service) replay: Option<santi_estate::ReplayDraft<'a>>,
}

struct Intake {
    outcome: ingest::Outcome,
    inserted: bool,
}

#[derive(Clone, Copy)]
struct Drive<'a> {
    trigger: &'a str,
    inbox: Option<&'a str>,
    operation: &'a str,
}

impl Service {
    pub(in crate::service) async fn notify(
        &self,
        attention: santi_estate::AttentionDraft<'_>,
        notice: santi_estate::NoticeDraft<'_>,
    ) -> Result<(), String> {
        let strand = notice.strand.to_string();
        let offered = self.store.attend_job(attention, notice, 500).await?;
        self.dispatched().await;
        if offered.inserted
            && let Some(inbox) = offered.inbox
        {
            self.inboxes.lock().unwrap().insert(strand, inbox);
        }
        Ok(())
    }

    pub(in crate::service) async fn rouse(&self) {
        let pending = std::mem::take(&mut *self.inboxes.lock().unwrap());
        for (strand, inbox) in pending {
            let outcome = self
                .poke(&strand, "system", Some(&inbox), "inbox_notice_poke")
                .await;
            if let drive::Outcome::Failed(error) = outcome {
                eprintln!(
                    "santi: inbox notice wake failed strand={} code={} detail={}",
                    strand, error.code, error.message
                );
            }
        }
    }

    pub async fn ingest(
        &self,
        selector: strand::Selector,
        content: message::Content,
        kind: message::Kind,
        trigger: &str,
    ) -> Result<ingest::Outcome, String> {
        self.accept(
            selector,
            Ingest {
                content,
                kind,
                trigger,
                source: None,
                replay: None,
            },
        )
        .await
    }

    pub(in crate::service) async fn accept(
        &self,
        selector: strand::Selector,
        input: Ingest<'_>,
    ) -> Result<ingest::Outcome, String> {
        let strand = self.store.selected(&selector, &crate::now()).await?;
        let outcome = self.enqueue(&strand, input).await?;
        Ok(outcome)
    }

    async fn enqueue(&self, strand: &Strand, input: Ingest<'_>) -> Result<ingest::Outcome, String> {
        let trigger = input.trigger.to_string();
        let intake = self.intake(&strand.id, input).await?;
        if intake.inserted
            && let ingest::Outcome::Accepted { receipt } = &intake.outcome
        {
            let service = self.clone();
            let strand = strand.id.clone();
            let inbox = receipt.inbox.clone();
            let context = self.context.clone();
            tokio::spawn(context.carry(async move {
                service.dispatched().await;
                service
                    .poke(&strand, &trigger, Some(&inbox), "ingest_poke")
                    .await;
            }));
        }
        Ok(intake.outcome)
    }

    async fn intake(&self, strand: &str, input: Ingest<'_>) -> Result<Intake, String> {
        let inbox = crate::tag("inbox");
        let created = crate::now();
        let draft = santi_estate::InboxDraft {
            tag: &inbox,
            strand,
            kind: input.kind,
            content: &input.content,
            source: input.source.as_ref(),
            created: &created,
        };
        match input.replay {
            Some(replay) => {
                let accepted = self.store.accept_replay(draft, replay, 500).await?;
                Ok(Intake {
                    outcome: ingest::Outcome::Accepted {
                        receipt: accepted.receipt,
                    },
                    inserted: accepted.inserted,
                })
            }
            None => {
                self.store.accept_inbox(draft, 500).await?;
                Ok(Intake {
                    outcome: ingest::Outcome::Accepted {
                        receipt: ingest::Receipt {
                            strand: strand.to_string(),
                            inbox,
                            warning: None,
                        },
                    },
                    inserted: true,
                })
            }
        }
    }
}

mod dispatch;
mod external;
mod face;
mod failure;

pub(super) fn erred(descriptor: santi_error::Descriptor, strand: &str, message: String) -> Fault {
    engine().transient(crate::Signal {
        descriptor,
        source: santi_error::Source::new("santi-core", "strand_send"),
        scope: Some(santi_error::Scope::new("strand", strand)),
        message,
        context: serde_json::Value::Null,
    })
}
