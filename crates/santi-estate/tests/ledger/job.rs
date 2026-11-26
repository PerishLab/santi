use santi_estate::{Call, Capability, Effect, Job, Prepared, Store};

use super::seed::{FIRST, SOUL};

pub(super) async fn job(store: &Store, strand: &str, turn: &str) {
    let id = |name: &str| format!("{name}_{strand}");
    let call = store
        .calling(Call {
            tag: &id("job_call"),
            turn,
            tool: "job",
            arguments: &serde_json::json!({"command": "true"}),
            created: FIRST,
        })
        .await
        .expect("job call");
    let effect = store
        .stage(Effect {
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
        .grant(Capability {
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
        .submit(
            &digest,
            Job {
                environment: None,
                tag: &id("job"),
                description: "test job",
                command: "true",
                cwd: None,
                timeout: 30,
                limit: 1024,
                remind: None,
                notify: false,
                digest: "request_digest",
                generation: &id("stamp"),
                supervisor: &id("supervisor"),
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
