use super::*;

pub fn render(event: &str, data: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(data).ok()?;
    let beat = value
        .get("payload")
        .and_then(|payload| payload.get("beat"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    match (event, beat.as_str()) {
        ("open", _) | ("message", "delta") | ("thinking", "updated") => None,
        ("turn", "started") => {
            let turn = value.get("payload")?.get("turn")?;
            let id = turn.get("id").and_then(serde_json::Value::as_str)?;
            let trigger = turn
                .get("trigger")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            Some(format!("turn started {id} ({trigger})"))
        }
        ("turn", "active") => {
            let activity = value.get("payload")?.get("activity")?;
            let id = activity.get("turn").and_then(serde_json::Value::as_str)?;
            let state = activity
                .get("state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            Some(format!("turn {id}: {state}"))
        }
        ("message", "created") => {
            let message = value.get("payload")?.get("message")?;
            Some(format!(
                "message {} {}: {}",
                sequence(message),
                speaker(message),
                snippet(
                    message
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default(),
                    160,
                )
            ))
        }
        ("message", "completed") => {
            let payload = value.get("payload")?;
            let turn = payload
                .get("turn")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let text = payload
                .get("message")
                .and_then(|message| message.get("text"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            Some(format!(
                "assistant completed {turn}: {}",
                snippet(text, 500)
            ))
        }
        ("tool", "called") => {
            let call = value.get("payload")?.get("call")?;
            let id = call
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let name = call
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            Some(format!("tool call {name} ({id})"))
        }
        ("tool", "replied") => {
            let result = value.get("payload")?.get("result")?;
            let call = result
                .get("call")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let status = if result.get("error").is_some_and(|error| !error.is_null()) {
                "error"
            } else {
                "ok"
            };
            Some(format!("tool result {call}: {status}"))
        }
        ("thinking", "created") => thinking(&value, "thinking started"),
        ("thinking", "completed") => thinking(&value, "thinking completed"),
        ("material", "updated") => {
            let material = value.get("payload")?.get("material")?;
            let kind = material
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("material");
            Some(format!("material updated {kind}"))
        }
        ("turn", "completed") => {
            field(data, &["payload", "turn"]).map(|id| format!("turn completed {id}"))
        }
        ("turn", "failed") => failed(value.get("payload")?),
        ("transition", _) => transition(value.get("payload")?),
        _ => Some(format!("{event}: {}", snippet(data, 240))),
    }
}

fn failed(payload: &serde_json::Value) -> Option<String> {
    let id = payload
        .get("turn")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let error = payload.get("error")?;
    let code = error
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown error");
    let incident = error
        .get("incident")
        .and_then(serde_json::Value::as_str)
        .map(|id| format!(" (incident {id})"))
        .unwrap_or_default();
    Some(format!(
        "turn failed {id}: {}",
        snippet(&format!("{code}: {message}{incident}"), 240)
    ))
}

fn transition(payload: &serde_json::Value) -> Option<String> {
    let transition = payload.get("transition")?;
    let kind = transition
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("changed");
    let incident = transition.get("held")?;
    let id = incident
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let code = incident
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    Some(format!("error {kind} {code} ({id})"))
}

fn thinking(value: &serde_json::Value, label: &str) -> Option<String> {
    let thinking = value.get("payload")?.get("thinking")?;
    let id = thinking
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let turn = thinking
        .get("turn")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    Some(format!("{label} {id} ({turn})"))
}

fn sequence(message: &serde_json::Value) -> String {
    message
        .get("relation")
        .and_then(|relation| relation.get("seq"))
        .and_then(serde_json::Value::as_i64)
        .map(|seq| format!("#{seq}"))
        .unwrap_or_else(|| "#?".to_string())
}

fn speaker(message: &serde_json::Value) -> String {
    let inner = message.get("message").unwrap_or(message);
    let actor = inner
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown");
    let kind = inner
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("text");
    format!("{actor}/{kind}")
}
