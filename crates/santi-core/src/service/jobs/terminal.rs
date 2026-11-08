use serde_json::json;
use sha2::{Digest, Sha256};

use super::Service;
use crate::{ingest, message, now};
use santi_estate::JobRecord;

pub(super) async fn finish(service: &Service, record: JobRecord) -> Result<(), String> {
    if !record.notify_on_terminal || record.terminal_notified.is_some() {
        return Ok(());
    }
    let Some(mut record) = service.store.job_record(&record.job.id).await? else {
        return Ok(());
    };
    if !record.job.state.terminal()
        || !record.notify_on_terminal
        || record.terminal_notified.is_some()
    {
        return Ok(());
    }
    let (stdout, stderr) = output(service, &record)?;
    for _ in 0..2 {
        let observed = now();
        let revision = record
            .attention_revision
            .checked_add(1)
            .ok_or_else(|| "job attention revision is out of range".to_string())?;
        let key = format!("job/{}/{}", record.job.id, record.generation);
        let content = fact(&record, &observed, stdout, stderr);
        let encoded = serde_json::to_vec(&content).map_err(|error| error.to_string())?;
        let digest = format!("{:x}", Sha256::digest(encoded));
        let source = ingest::Source::new("job")
            .with_ref(record.job.id.clone())
            .with_metadata(json!({
                "schema": "santi.job.attention.v1",
                "stamp": record.generation,
                "revision": revision,
                "observed_at": observed,
                "terminal": true,
            }));
        let inbox = crate::tag("inbox");
        service
            .notify(
                santi_estate::AttentionDraft {
                    job: &record.job.id,
                    base: record.attention_revision,
                    at: &observed,
                    runtime: false,
                    output: false,
                    reminded: false,
                    tick: record.reminder_tick,
                    next: None,
                    terminal: true,
                },
                santi_estate::NoticeDraft {
                    tag: &inbox,
                    strand: &record.job.origin.strand,
                    key: &key,
                    revision: i64::try_from(revision)
                        .map_err(|_| "job attention revision is out of range".to_string())?,
                    digest: &digest,
                    content: &content,
                    source: &source,
                    causes: &["terminal".to_string()],
                    created: &observed,
                },
            )
            .await?;
        let Some(after) = service.store.job_record(&record.job.id).await? else {
            return Ok(());
        };
        if after.terminal_notified.is_some() {
            return Ok(());
        }
        record = after;
        if !record.job.state.terminal() {
            return Ok(());
        }
    }
    Err("terminal job notification could not be recorded".to_string())
}

pub(super) async fn report(service: &Service, record: &JobRecord) {
    if let Err(error) = finish(service, record.clone()).await {
        eprintln!(
            "santi: terminal job notification failed job={} detail={error}",
            record.job.id
        );
    }
}

fn fact(record: &JobRecord, observed: &str, stdout: u64, stderr: u64) -> message::Content {
    let job = &record.job;
    message::Content::text(
        [
            "item_kind: job_terminal".to_string(),
            format!("job_id: {}", job.id),
            format!("stamp: {}", record.generation),
            format!("description: {:?}", job.description),
            format!("state: {}", job.state.encode()),
            format!("reason: {}", job.reason.as_deref().unwrap_or("none")),
            format!(
                "exit_code: {}",
                job.exit_code
                    .map_or_else(|| "none".to_string(), |exit| exit.to_string())
            ),
            format!("finished_at: {}", job.finished.as_deref().unwrap_or("none")),
            format!("observed_at: {observed}"),
            format!("stdout_bytes: {stdout}"),
            format!("stderr_bytes: {stderr}"),
            format!("combined_output_bytes: {}", stdout.saturating_add(stderr)),
            format!("log: job/{}/{}", job.id, record.generation),
            format!("origin_strand: {}", job.origin.strand),
            format!("origin_turn: {}", job.origin.turn),
        ]
        .join("\n"),
    )
}

pub(super) fn output(service: &Service, record: &JobRecord) -> Result<(u64, u64), String> {
    let directory = service.jobhome(record);
    Ok((
        size(&directory.join("stdout.log"))?,
        size(&directory.join("stderr.log"))?,
    ))
}

fn size(path: &std::path::Path) -> Result<u64, String> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.to_string()),
    }
}
