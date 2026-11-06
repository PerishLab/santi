use santi_core::{ingest, service::Service};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::process::Command;

pub(super) fn message(adaptor: &str, metadata: Option<&Value>) -> Option<String> {
    if adaptor != "feishu" {
        return None;
    }
    metadata?.get("message")?.as_str().map(str::to_string)
}

pub(super) async fn accept(service: &Service, message: Option<String>, receipt: &ingest::Receipt) {
    let Some(message) = message else {
        return;
    };
    let Some(profile) = crate::runtime::held().feishu.acknowledge.as_deref() else {
        return;
    };
    let intent = json!({"provider":"feishu","profile":profile,"message":message,"emoji":"Get"});
    match service.acknowledge(&receipt.inbox, &intent).await {
        Ok(true) => spawn(
            service.clone(),
            receipt.inbox.clone(),
            profile.to_string(),
            message,
        ),
        Ok(false) => {}
        Err(error) => eprintln!(
            "santi: acknowledgement not dispatched inbox={}: {error}",
            receipt.inbox
        ),
    }
}

fn spawn(service: Service, inbox: String, profile: String, message: String) {
    tokio::spawn(async move {
        let result = dispatch(&profile, &message).await;
        if let Err(error) = service.acknowledged(&inbox, &result).await {
            eprintln!("santi: acknowledgement result not committed inbox={inbox}: {error}");
        }
    });
}

async fn dispatch(profile: &str, message: &str) -> Value {
    let gate = match super::process::gate(profile) {
        Ok(gate) => gate,
        Err(reason) => return json!({"state":"unsent", "reason":reason}),
    };
    match gate.tokio().guard(|| invoke(profile, message)).await {
        Ok(result) => result,
        Err(reason) => json!({"state":"unknown", "reason":reason.to_string()}),
    }
}

async fn invoke(profile: &str, message: &str) -> Value {
    if profile.is_empty()
        || profile.len() > 64
        || !profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return json!({"state":"unsent","reason":"invalid acknowledgement profile"});
    }
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
        return json!({"state":"unsent","reason":"acknowledgement tool environment is unresolved"});
    }
    let mut command = Command::new("runseal");
    command.env_clear();
    santi_core::environment::allow(command.as_std_mut());
    command.envs(environment.values);
    command
        .args([
            &format!(":{profile}"),
            "@feishu",
            "--as",
            "bot",
            "--json",
            "im",
            "reactions",
            "create",
            "--message-id",
            message,
            "--emoji-type",
            "Get",
        ])
        .kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(30), command.output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let state = classify(&stdout, output.status.success(), message);
            json!({"state":state,"exit":output.status.code(),"stdout":stdout,"stderr":stderr})
        }
        Ok(Err(_)) => json!({"state":"unsent","reason":"runseal process could not start"}),
        Err(_) => {
            json!({"state":"unknown","reason":"runseal process timed out; observe exact reaction before recovery"})
        }
    }
}

fn classify(stdout: &str, success: bool, message: &str) -> &'static str {
    let Ok(value) = serde_json::from_str::<Value>(stdout) else {
        return "unknown";
    };
    let value = &value["feishu"];
    if success {
        if value["action"] != "im.reactions.create" || value["outcome"] != "accepted" {
            return "unknown";
        }
        if value["data"]["message"] != message
            || value["data"]["reaction"]["reaction_type"]["emoji_type"] != "Get"
        {
            return "unknown";
        }
        return "accepted";
    }
    match value["error"]["outcome"].as_str() {
        Some("unsent") => "unsent",
        Some("rejected") => "rejected",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes() {
        let reaction = json!({"reaction_type":{"emoji_type":"Get"}});
        let data = json!({"message":"om_test","reaction":reaction});
        let value =
            json!({"feishu":{"action":"im.reactions.create","outcome":"accepted","data":data}});
        assert_eq!(classify(&value.to_string(), true, "om_test"), "accepted");
        assert_eq!(classify(&value.to_string(), true, "om_other"), "unknown");
        assert_eq!(classify("incomplete", true, "om_test"), "unknown");
        let mut value = json!({"feishu":{"error":{"outcome":"unknown"}}});
        for state in ["unsent", "rejected", "unknown"] {
            value["feishu"]["error"]["outcome"] = json!(state);
            assert_eq!(classify(&value.to_string(), false, "om_test"), state);
        }
    }
}
