use santi_estate::{CallDraft, CapabilityDraft, EffectDraft, JobDraft, Prepared, Store};

use super::seed::{FIRST, SOUL};

pub(super) async fn job(store: &Store, strand: &str, turn: &str) {
    let id = |name: &str| format!("{name}_{strand}");
    let call = store
        .create_call(CallDraft {
            tag: &id("job_call"),
            turn,
            tool: "job",
            arguments: &serde_json::json!({"command": "true"}),
            created: FIRST,
        })
        .await
        .expect("job call");
    let effect = store
        .prepare_effect(EffectDraft {
            tag: &id("job_effect"),
            turn,
            call: Some(&call.id),
            kind: "job",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("job effect");
    let digest = id("capability");
    store
        .create_capability(CapabilityDraft {
            digest: &digest,
            expires: 1_000,
            soul: SOUL,
            strand,
            turn,
            call: &call.id,
            effect: &effect.id,
            created: FIRST,
        })
        .await
        .expect("capability");
    let Prepared::New(_) = store
        .prepare_job(
            &digest,
            JobDraft {
                environment: None,
                tag: &id("job"),
                description: "test job",
                command: "true",
                cwd: None,
                timeout_seconds: 30,
                output_limit_bytes: 1024,
                remind_every_seconds: None,
                request_sha256: "request_digest",
                generation: &id("stamp"),
                supervisor_ref: &id("supervisor"),
                created: FIRST,
            },
            500,
        )
        .await
        .expect("job")
    else {
        panic!("job must be new");
    };
}
