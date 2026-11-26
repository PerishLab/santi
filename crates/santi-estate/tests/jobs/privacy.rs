use santi_estate::{Environment, Record, Store};

pub async fn verify(path: &std::path::Path, replayed: &Record, environment: &Environment) {
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
            .tracked("job_test")
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
        .grant(santi_estate::Capability {
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
    assert_eq!(store.sweep(11).await.expect("expire capabilities"), 1);
}
