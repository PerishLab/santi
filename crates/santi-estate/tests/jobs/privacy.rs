use santi_estate::{Environment, JobRecord, Store};

pub async fn verify(path: &std::path::Path, replayed: &JobRecord, environment: &Environment) {
    assert_eq!(replayed.environment.as_ref(), Some(environment));
    assert!(!format!("{replayed:?}").contains("frozen-secret-probe"));
    assert!(
        !serde_json::to_string(&replayed.job)
            .expect("public job")
            .contains("PRIVATE_PROBE")
    );
    let retained = Store::bootstrap(path, super::SUDO).await.expect("reopen");
    assert_eq!(
        retained
            .job_record("job_test")
            .await
            .expect("read")
            .expect("record")
            .environment,
        Some(environment.clone())
    );
}

pub async fn expire(store: &Store, origin: [&str; 4]) {
    let [strand, turn, call, effect] = origin;
    store
        .create_capability(santi_estate::CapabilityDraft {
            digest: "expired_capability",
            expires: 10,
            soul: "soul_test",
            strand,
            turn,
            call,
            effect,
            created: super::FIRST,
        })
        .await
        .expect("expiring capability");
    assert_eq!(
        store
            .expire_capabilities(11)
            .await
            .expect("expire capabilities"),
        1
    );
}
