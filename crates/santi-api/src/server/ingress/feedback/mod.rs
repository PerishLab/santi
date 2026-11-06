use futures_util::{StreamExt, stream};
use receipt::reply::{Binding, Owner, State, Status};
use santi_core::{receipt, service::Service};
use serde_json::{Value, json};
use std::time::Duration;
mod dispatch;
mod render;
pub(in crate::server) mod routes;
pub(in crate::server) use routes::{inspect, reply, resolve};

pub(super) fn binding(adaptor: &str, metadata: Option<&Value>) -> Option<Binding> {
    if adaptor != "feishu" {
        return None;
    }
    let config = crate::runtime::held().feishu.feedback.as_ref()?;
    let metadata = metadata?;
    Some(Binding {
        adaptor: "feishu".into(),
        delay: 3000,
        context: json!({"profile":config.profile, "app":config.app,
            "parent":metadata.get("message")?.as_str()?, "chat":metadata.get("chat_id")?.as_str()?}),
    })
}

pub(in crate::server) async fn watch(service: Service) {
    while !service.closing() {
        match service.replies().await {
            Ok(statuses) => {
                let service = &service;
                stream::iter(statuses)
                    .for_each_concurrent(4, |status| process(service, status))
                    .await;
            }
            Err(error) => eprintln!("santi: feedback observation failed: {error}"),
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn advance(service: &Service, status: &Status) -> Result<(), String> {
    let attempts = &status.window.attempts;
    if attempts
        .iter()
        .any(|a| matches!(a.state, State::Dispatching | State::Unknown))
    {
        return Ok(());
    }
    let queued = attempts.iter().any(|a| a.state == State::Queued);
    if status.window.owner == Owner::Soul && !queued {
        return Ok(());
    }
    if status.window.owner == Owner::Runtime {
        let task = service
            .receipt(&status.inbox)
            .await?
            .ok_or("feedback receipt missing")?;
        let elapsed = elapsed(&status.accepted)?;
        let completed = task.state == receipt::State::Completed;
        if elapsed >= status.window.binding.delay && (!completed || status.window.message.is_some())
        {
            let sequence = task
                .transitions
                .last()
                .ok_or("feedback transition missing")?
                .sequence;
            let operation = format!("runtime:{sequence}");
            if attempts.iter().any(|a| a.operation == operation) && !queued {
                return Ok(());
            }
            service
                .feedback(&status.inbox, &operation, &render::automatic(&task))
                .await?;
        }
    }
    let profile = status.window.binding.context["profile"]
        .as_str()
        .ok_or("feedback profile missing")?;
    let gate = super::process::gate(profile)?;
    gate.tokio()
        .guard(|| send(service, status))
        .await
        .map_err(|e| e.to_string())?
}

async fn send(service: &Service, status: &Status) -> Result<(), String> {
    if let Some(attempt) = service.outgoing(&status.inbox).await? {
        let result = dispatch::call(&status.window.binding, &attempt).await;
        service
            .delivered(
                &status.inbox,
                santi_estate::Outcome {
                    operation: &attempt.operation,
                    message: result["message"].as_str(),
                    result: &result,
                },
            )
            .await?;
    }
    Ok(())
}

fn elapsed(accepted: &str) -> Result<u64, String> {
    let accepted: jiff::Timestamp = accepted.parse().map_err(|e: jiff::Error| e.to_string())?;
    let millis = jiff::Timestamp::now()
        .as_millisecond()
        .saturating_sub(accepted.as_millisecond());
    Ok(millis.max(0) as u64)
}

async fn process(service: &Service, status: Status) {
    if let Err(error) = advance(service, &status).await {
        eprintln!(
            "santi: feedback window unavailable inbox={}: {error}",
            status.inbox
        );
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod surface;
