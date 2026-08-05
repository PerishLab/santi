use std::time::{Duration, UNIX_EPOCH};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::LABEL;
use crate::service::Service;
use crate::{ingest, message, strand};

pub(super) async fn regular(service: &Service, soul: &str, revision: i64) -> Result<(), String> {
    let observed = crate::stamped(UNIX_EPOCH + Duration::from_millis(revision as u64))?;
    let strand = selected(service, soul, &observed).await?;
    let content = message::Content::text(format!("current_time: {observed}"));
    let encoded = serde_json::to_vec(&content).map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(encoded));
    let source = ingest::Source::new("clock")
        .with_ref(soul.to_string())
        .with_metadata(json!({"schema": "santi.soul.clock.v1"}));
    let key = format!("clock/{soul}");
    let tag = crate::tag("inbox");
    let offered = service
        .store
        .offer_notice(
            santi_estate::NoticeDraft {
                tag: &tag,
                strand: &strand.id,
                key: &key,
                revision,
                digest: &digest,
                content: &content,
                source: &source,
                causes: &[],
                created: &observed,
            },
            500,
        )
        .await?;
    service.dispatched().await;
    if offered.inserted
        && let Some(inbox) = offered.inbox
    {
        service.inboxes.lock().unwrap().insert(strand.id, inbox);
    }
    Ok(())
}

pub(super) async fn leased(
    service: &Service,
    lease: &santi_estate::WakeLease,
    now_millis: i64,
) -> Result<(), String> {
    let due = lease
        .next_millis
        .ok_or_else(|| "active wake lease has no next time".to_string())?;
    let observed = crate::stamped(
        UNIX_EPOCH
            + Duration::from_millis(
                u64::try_from(now_millis)
                    .map_err(|_| "wake lease observation time is out of range".to_string())?,
            ),
    )?;
    let strand = selected(service, &lease.soul, &observed).await?;
    let remaining = lease.remaining.saturating_sub(1);
    let content = message::Content::text(
        [
            format!("current_time: {observed}"),
            "item_kind: wake_lease".to_string(),
            format!("wake_lease_generation: {}", lease.generation),
            format!("wake_rounds_remaining_after_this: {remaining}"),
            format!(
                "wake_lease_action: call wake action=renew generation={} to continue for {} more rounds, or action=silence to stop now",
                lease.generation,
                crate::wake::ROUNDS
            ),
        ]
        .join("\n"),
    );
    let encoded = serde_json::to_vec(&content).map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(encoded));
    let source = ingest::Source::new("clock")
        .with_ref(lease.soul.clone())
        .with_metadata(json!({
            "schema": "santi.soul.wake-lease.v1",
            "generation": lease.generation,
            "remaining_after": remaining,
            "scheduled_millis": due,
        }));
    let key = format!("clock/wake/{}", lease.soul);
    let tag = crate::tag("inbox");
    let offered = service
        .store
        .offer_wake(santi_estate::WakeOfferDraft {
            soul: &lease.soul,
            generation: lease.generation,
            due_millis: due,
            now_millis,
            occurred: &observed,
            notice: santi_estate::NoticeDraft {
                tag: &tag,
                strand: &strand.id,
                key: &key,
                revision: due,
                digest: &digest,
                content: &content,
                source: &source,
                causes: &[],
                created: &observed,
            },
            gate: 500,
        })
        .await?;
    if let Some(offered) = offered {
        service.dispatched().await;
        if offered.inserted
            && let Some(inbox) = offered.inbox
        {
            service.inboxes.lock().unwrap().insert(strand.id, inbox);
        }
    }
    Ok(())
}

async fn selected(
    service: &Service,
    soul: &str,
    observed: &str,
) -> Result<crate::strand::Strand, String> {
    service
        .store
        .selected(
            &strand::Selector::ByLabel {
                soul: soul.to_string(),
                label: LABEL.to_string(),
            },
            observed,
        )
        .await
}
