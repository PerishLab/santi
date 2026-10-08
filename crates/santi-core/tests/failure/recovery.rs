use super::*;

#[tokio::test]
async fn recovers() {
    let temp = tempfile::tempdir().expect("temp");
    let provider = Arc::new(FailureProvider {
        fail_with: Some("temporary provider outage".to_string()),
        fail_for_requests: Some(1),
        ..FailureProvider::default()
    });
    let service = open_service(&temp, provider.clone()).await;
    let strand = service.weave().await.expect("strand").strand;
    let accepted = send_text(&service, &strand.id, "recover original accepted text").await;
    wait_for_turn(
        &service,
        &strand.id,
        &turn(&accepted).id,
        turn::Status::Failed,
    )
    .await;
    let prior = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    service.resume().await.expect("automatic resume");
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let driven = service.drive(&strand.id).await.expect("explicit drive");
    assert_eq!(driven.state, santi_core::drive::State::Started);
    let retried = driven.turn.expect("new turn");
    assert_ne!(retried.id, turn(&accepted).id);
    let runtime = wait_for_turn(&service, &strand.id, &retried.id, turn::Status::Completed).await;
    let status = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    assert_eq!(status.state, santi_core::receipt::State::Completed);
    assert_eq!(status.accepted, prior.accepted);
    assert_eq!(
        runtime
            .messages
            .iter()
            .filter(|placed| placed.text == "recover original accepted text")
            .count(),
        1
    );
    assert!(runtime.effects.is_empty());
    assert_eq!(
        service
            .drive(&strand.id)
            .await
            .expect("settled drive")
            .state,
        santi_core::drive::State::Idle
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn refuses() {
    let temp = tempfile::tempdir().expect("temp");
    let provider = Arc::new(FailureProvider {
        fail_with: Some("interrupted".to_string()),
        ..FailureProvider::default()
    });
    let service = open_service(&temp, provider.clone()).await;
    let strand = service.weave().await.expect("strand").strand;
    let accepted = send_text(&service, &strand.id, "external effect").await;
    wait_for_turn(
        &service,
        &strand.id,
        &turn(&accepted).id,
        turn::Status::Failed,
    )
    .await;
    drop(service);
    let store = santi_core::Store::open(temp.path().join("santi.sqlite"))
        .await
        .expect("store");
    store
        .prepare_invocation(
            santi_estate::CallDraft {
                tag: "call_refusal",
                turn: &turn(&accepted).id,
                tool: "shell",
                arguments: &serde_json::json!({"command": "external effect"}),
                created: &santi_core::now(),
            },
            Some(santi_estate::EffectDraft {
                tag: "effect_refusal",
                turn: &turn(&accepted).id,
                call: Some("call_refusal"),
                kind: "shell",
                metadata: None,
                created: &santi_core::now(),
            }),
        )
        .await
        .expect("effect invocation");
    store
        .dispatch_effect("effect_refusal", &santi_core::now())
        .await
        .expect("dispatch");
    store
        .unknown_effect("effect_refusal", "capture interrupted", &santi_core::now())
        .await
        .expect("unknown");
    drop(store);
    let service = reopened(&temp, provider.clone()).await;
    let before = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    let unknown = service
        .drive(&strand.id)
        .await
        .expect_err("unknown refusal");
    guidance(&unknown, &accepted.receipt.inbox, "unknown");
    assert_eq!(
        serde_json::to_value(&before).expect("before JSON"),
        serde_json::to_value(
            service
                .receipt(&accepted.receipt.inbox)
                .await
                .expect("receipt")
                .expect("held")
        )
        .expect("after JSON")
    );
    assert!(
        service
            .settle("effect_refusal", santi_core::effect::Outcome::Applied, "")
            .await
            .is_err()
    );
    service
        .settle(
            "effect_refusal",
            santi_core::effect::Outcome::Applied,
            "operator confirmed execution",
        )
        .await
        .expect("resolve");
    let before = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    let applied = service
        .drive(&strand.id)
        .await
        .expect_err("applied refusal");
    guidance(&applied, &accepted.receipt.inbox, "settled_applied");
    assert_eq!(
        serde_json::to_value(&before).expect("before JSON"),
        serde_json::to_value(
            service
                .receipt(&accepted.receipt.inbox)
                .await
                .expect("receipt")
                .expect("held")
        )
        .expect("after JSON")
    );
    let rejected = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "do not accept".into(),
                }],
            },
        )
        .await
        .expect_err("admission guard");
    guidance(&rejected, &accepted.receipt.inbox, "settled_applied");
    drop(service);
    let reopened = reopened(&temp, provider.clone()).await;
    reopened.resume().await.expect("resume");
    let runtime = reopened
        .snapshot(&strand.id)
        .await
        .expect("snapshot")
        .expect("strand");
    assert_eq!(runtime.turns.len(), 1);
    assert_eq!(runtime.calls.len(), 1);
    assert_eq!(runtime.effects.len(), 1);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let held = runtime
        .errors
        .iter()
        .find(|error| error.code == "runtime.strand.drive_failed")
        .expect("incident");
    assert_eq!(held.latest.context["recovery"], applied.context["recovery"]);
    let refusal = reopened
        .drive(&strand.id)
        .await
        .expect_err("retained refusal");
    guidance(&refusal, &accepted.receipt.inbox, "settled_applied");
}

fn guidance(error: &santi_core::Fault, inbox: &str, state: &str) {
    assert_eq!(error.code, "runtime.strand.drive_failed");
    assert_eq!(
        error.context["recovery"]["command"],
        "santi effect query effect_refusal"
    );
    assert_eq!(
        error.context["recovery"]["receipt"],
        format!("santi receipt {inbox}")
    );
    assert_eq!(error.context["recovery"]["state"], state);
    assert_eq!(error.context["recovery"]["resend"], false);
    let detail = error.context["detail"].as_str().expect("detail");
    let instruction = error.context["recovery"]["instruction"]
        .as_str()
        .expect("instruction");
    assert!(detail.contains(instruction));
    assert!(detail.contains("do not resend or repeat the command"));
    if state == "settled_applied" {
        assert!(instruction.contains("replay remains prohibited"));
    } else {
        assert!(instruction.contains("evidenced outcome"));
        assert!(instruction.contains("confirmed not-applied"));
    }
}

async fn reopened(temp: &tempfile::TempDir, provider: Arc<FailureProvider>) -> Service {
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .expect("reopen")
}
