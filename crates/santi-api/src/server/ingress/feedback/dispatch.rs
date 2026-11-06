use santi_core::receipt::reply::{Attempt, Binding};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::process::Command;

pub(super) async fn call(binding: &Binding, attempt: &Attempt) -> Value {
    let (mut command, _private) = match prepare(binding, attempt) {
        Ok(prepared) => prepared,
        Err(reason) => return json!({"state":"unsent", "reason":reason}),
    };
    match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let (state, message) = classify(&stdout, output.status.success(), binding, attempt);
            json!({"state":state,"message":message,"exit":output.status.code(),"stdout":stdout,"stderr":stderr})
        }
        Ok(Err(_)) => json!({"state":"unsent","reason":"runseal process could not start"}),
        Err(_) => {
            json!({"state":"unknown","reason":"process timed out; exact remote settlement required"})
        }
    }
}

fn prepare(binding: &Binding, attempt: &Attempt) -> Result<(Command, tempfile::TempDir), String> {
    let context = &binding.context;
    let profile = context["profile"]
        .as_str()
        .ok_or("feedback profile missing")?;
    let app = context["app"].as_str().ok_or("feedback app missing")?;
    crate::runtime::Feedback::new(profile.into(), app.into())?;
    if binding.adaptor != "feishu" {
        return Err("unsupported reply adaptor".into());
    }
    let parent = context["parent"]
        .as_str()
        .ok_or("feedback parent missing")?;
    let private = tempfile::tempdir().map_err(|e| e.to_string())?;
    let path = private.path().join("post.json");
    write(&path, &attempt.body)?;
    let mut command = Command::new("runseal");
    command.env_clear();
    santi_core::environment::allow(command.as_std_mut());
    let declared = crate::runtime::held()
        .environment
        .iter()
        .filter(|(key, _)| ["PATH", "RUNSEAL_HOME"].contains(&key.as_str()))
        .map(|(key, value)| santi_core::environment::Declaration {
            scope: "global".into(),
            name: key.clone(),
            value: value.clone(),
        });
    let environment = santi_core::environment::resolve(declared, &crate::config::env);
    if !environment.unresolved.is_empty() {
        return Err("feedback tool environment unresolved".into());
    }
    command.envs(environment.values);
    command.args([
        &format!(":{profile}"),
        "@feishu",
        "--as",
        "bot",
        "--app",
        app,
        "--json",
        "im",
        "messages",
    ]);
    match attempt.target.as_deref() {
        Some(message) => {
            command.args(["edit", "--message-id", message]);
        }
        None => {
            command.args([
                "reply",
                "--message-id",
                parent,
                "--idempotency-key",
                &attempt.uuid,
            ]);
        }
    }
    command.arg("--post-file").arg(path).kill_on_drop(true);
    Ok((command, private))
}

fn write(path: &std::path::Path, body: &Value) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(body).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub(super) fn classify(
    stdout: &str,
    success: bool,
    binding: &Binding,
    attempt: &Attempt,
) -> (&'static str, Option<String>) {
    let Ok(value) = serde_json::from_str::<Value>(stdout) else {
        return ("unknown", None);
    };
    let value = &value["feishu"];
    if !success {
        return match value["error"]["outcome"].as_str() {
            Some("unsent") => ("unsent", None),
            Some("rejected") => ("rejected", None),
            _ => ("unknown", None),
        };
    }
    let action = if attempt.target.is_some() {
        "im.messages.edit"
    } else {
        "im.messages.reply"
    };
    if value["action"] != action || value["outcome"] != "accepted" {
        return ("unknown", None);
    }
    if value["identity"]["app"] != binding.context["app"] || !value["identity"]["user"].is_null() {
        return ("unknown", None);
    }
    let data = &value["data"];
    if data["chat_id"] != binding.context["chat"] || data["msg_type"] != "post" {
        return ("unknown", None);
    }
    let Some(message) = data["message_id"].as_str().filter(|s| s.starts_with("om_")) else {
        return ("unknown", None);
    };
    if let Some(target) = &attempt.target {
        if target != message {
            return ("unknown", None);
        }
    } else if data["parent_id"] != binding.context["parent"] {
        return ("unknown", None);
    }
    if data["sender"]["id"] != binding.context["app"] || data["sender"]["sender_type"] != "app" {
        return ("unknown", None);
    }
    ("accepted", Some(message.into()))
}
