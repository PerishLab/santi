use crate::service::Service;
use crate::{Fault, Ruled, catalog, engine};

use super::Drive;

impl Service {
    pub(in crate::service) async fn gated(
        &self,
        strand: &str,
        operation: &str,
    ) -> Result<Option<Fault>, String> {
        let key = crate::drive::Error::Failed
            .descriptor()
            .key("strand", strand);
        let Some(incident) = self.store.incident(&key).await? else {
            return Ok(None);
        };
        let pending = self.store.inboxes(strand).await?.len();
        let mut draft = drive_failure(
            strand,
            Drive {
                trigger: "admission_guard",
                inbox: None,
                operation,
            },
            "strand driver recovery is still required",
            pending,
        );
        if incident.latest.context["recovery"]["effect"].is_string() {
            draft.context["recovery"] = incident.latest.context["recovery"].clone();
            draft.context["detail"] = incident.latest.context["detail"].clone();
        }
        self.store.raise(draft, &crate::now()).await.map(Some)
    }

    pub(super) async fn stumbled(&self, strand: &str, drive: Drive<'_>, detail: String) -> Fault {
        let pending = self
            .store
            .inboxes(strand)
            .await
            .map(|pending| pending.len())
            .unwrap_or_default();
        self.recorded(
            strand,
            drive,
            drive_failure(strand, drive, &detail, pending),
        )
        .await
    }

    pub(super) async fn declined(
        &self,
        strand: &str,
        drive: Drive<'_>,
        refusal: santi_estate::Refusal,
    ) -> Fault {
        let pending = self
            .store
            .inboxes(strand)
            .await
            .map(|rows| rows.len())
            .unwrap_or_default();
        let draft = refused(strand, drive, &refusal, pending);
        self.recorded(strand, drive, draft).await
    }

    async fn recorded(&self, strand: &str, drive: Drive<'_>, draft: santi_error::Draft) -> Fault {
        self.degrade();
        let error = match self.store.raise(draft, &crate::now()).await {
            Ok(error) => error,
            Err(persistence_error) => engine().transient(crate::Signal {
                descriptor: catalog::UNSAVED,
                source: santi_error::Source::new("santi-core", "strand_drive_failure"),
                scope: Some(santi_error::Scope::new("strand", strand)),
                message: "failed to persist strand driver incident".to_string(),
                context: serde_json::json!({
                    "accepted_before_failure": drive.inbox.is_some(),
                    "inbox": drive.inbox,
                    "detail": persistence_error,
                }),
            }),
        };
        eprintln!(
            "santi: strand drive failed code={} incident_id={} strand={} operation={} accepted_before_failure={}",
            error.code,
            error.incident.as_deref().unwrap_or("-"),
            strand,
            drive.operation,
            drive.inbox.is_some(),
        );
        self.dispatched().await;
        error
    }
}

fn drive_failure(
    strand: &str,
    drive: Drive<'_>,
    detail: &str,
    pending: usize,
) -> santi_error::Draft {
    santi_error::Draft {
        key: crate::drive::Error::Failed
            .descriptor()
            .key("strand", strand),
        descriptor: crate::drive::Error::Failed.descriptor(),
        scope: santi_error::Scope::new("strand", strand),
        source: santi_error::Source::new("santi-core", drive.operation),
        message: "strand driver could not start pending work".to_string(),
        context: serde_json::json!({
            "schema": "santi.error.strand_drive.v1",
            "accepted_before_failure": drive.inbox.is_some(),
            "inbox": drive.inbox,
            "pending_count": pending,
            "trigger": drive.trigger,
            "detail": bounded(detail),
            "recovery": {
                "command": format!("santi strand drive {strand}"),
                "resend": false,
            },
        }),
    }
}

fn refused(
    strand: &str,
    drive: Drive<'_>,
    refusal: &santi_estate::Refusal,
    pending: usize,
) -> santi_error::Draft {
    let inspection = format!("santi effect query {}", refusal.effect);
    let receipt = format!("santi receipt {}", refusal.inbox);
    let instruction = if refusal.state == "settled_applied" {
        "the effect was already applied; replay remains prohibited"
    } else {
        "reconcile external evidence and resolve the effect with an evidenced outcome; only a confirmed not-applied outcome permits replay"
    };
    let detail = format!(
        "receipt {} cannot be replayed while effect {} is {}; inspect `{receipt}` and `{inspection}`; {instruction}; do not resend or repeat the command",
        refusal.inbox, refusal.effect, refusal.state,
    );
    let mut draft = drive_failure(strand, drive, &detail, pending);
    draft.context["recovery"] = serde_json::json!({
        "command": inspection,
        "receipt": receipt,
        "effect": refusal.effect,
        "state": refusal.state,
        "instruction": instruction,
        "resend": false,
    });
    draft
}

fn bounded(detail: &str) -> String {
    const LIMIT: usize = 4096;
    if detail.len() <= LIMIT {
        return detail.to_string();
    }
    let suffix = " [truncated]";
    let mut end = LIMIT.saturating_sub(suffix.len());
    while end > 0 && !detail.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &detail[..end], suffix)
}
