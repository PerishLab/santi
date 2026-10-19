use super::super::*;
use santi_core::{service::JobSupervisor, strand::Strand};
use santi_estate::{CapabilityDraft, TurnDraft};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const CAPABILITY: &str = "jobcap_http_probe";

pub(super) async fn prepare(
    temp: &tempfile::TempDir,
    supervisor: Arc<dyn JobSupervisor>,
) -> (Service, Strand, santi_core::Store) {
    let database = temp.path().join("santi.sqlite");
    super::support::bootstrap(&database).await;
    let service = Service::supervised(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
        supervisor,
    )
    .await
    .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    let capability = CAPABILITY;
    let digest = format!("{:x}", Sha256::digest(capability.as_bytes()));
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
        + 60_000;
    let store = santi_core::Store::open(&database)
        .await
        .expect("open estate");
    store
        .create_turn(TurnDraft {
            tag: "turn_http",
            strand: &strand.id,
            trigger: santi_core::turn::Trigger::System,
            source: None,
            from: 0,
            created: &santi_core::now(),
        })
        .await
        .expect("turn");
    store
        .create_call(CallDraft {
            tag: "call_http",
            turn: "turn_http",
            tool: "shell",
            arguments: &serde_json::json!({"command": "printf ok"}),
            created: &santi_core::now(),
        })
        .await
        .expect("call");
    store
        .prepare_effect(EffectDraft {
            tag: "effect_http",
            turn: "turn_http",
            call: Some("call_http"),
            kind: "shell",
            metadata: None,
            created: &santi_core::now(),
        })
        .await
        .expect("effect");
    store
        .create_capability(CapabilityDraft {
            digest: &digest,
            expires: expiry,
            soul: &strand.soul,
            strand: &strand.id,
            turn: "turn_http",
            call: "call_http",
            effect: "effect_http",
            created: &santi_core::now(),
        })
        .await
        .expect("seed capability");
    (service, strand, store)
}
