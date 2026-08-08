use crate::service::Service;
use crate::{message, stream};

const ATTEMPTS: usize = 3;

impl Service {
    pub(in crate::service::tools) fn refused(&self, strand: &str) -> usize {
        let mut held = self.refusals.lock().unwrap();
        let count = held.entry(strand.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    pub(in crate::service::tools) fn relieved(&self, strand: &str) {
        self.refusals.lock().unwrap().remove(strand);
    }

    pub(in crate::service::tools) async fn escalated(
        &self,
        strand: &str,
        detail: &str,
    ) -> Result<(), String> {
        let content = message::Content::text(spelled(detail));
        let placed = self
            .store
            .place(santi_estate::MessageDraft {
                tag: &crate::tag("msg"),
                strand,
                actor: message::Role::System,
                actor_id: crate::SYSTEM,
                kind: message::Kind::SantiSystem,
                content: &content,
                state: message::State::Fixed,
                request: false,
                created: &crate::now(),
            })
            .await?;
        self.publish(
            strand,
            stream::Payload::Message(message::Beat::Created { message: placed }),
        );
        Ok(())
    }
}

pub(in crate::service::tools) fn exhausted(count: usize) -> bool {
    count >= ATTEMPTS
}

fn spelled(detail: &str) -> String {
    [
        "<system_message>".to_string(),
        "kind: context_block_escalated".to_string(),
        "scope: strand_local".to_string(),
        "wake: false".to_string(),
        "obligation: true".to_string(),
        format!("attempts: {ATTEMPTS}"),
        format!("condition: {detail}"),
        "summary: The context ceiling refused this strand three times without being resolved, so the turn was ended and the operator now holds it. Nothing was compacted or altered on your behalf.".to_string(),
        "</system_message>".to_string(),
    ]
    .join("\n")
}
