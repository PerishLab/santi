use super::*;
use crate::message;
use std::sync::Arc;

struct Unused;

#[async_trait::async_trait]
impl santi_provider::Provider for Unused {
    fn metadata(&self) -> santi_provider::Metadata {
        santi_provider::Metadata {
            provider: Arc::from("unused"),
            model: "unused".into(),
            budget: Some(santi_provider::Cap {
                bytes: 50_000,
                maintenance: 100_000,
                source: "test".into(),
            }),
        }
    }
    async fn stream(
        &self,
        _: santi_provider::Request,
    ) -> Result<santi_provider::Streaming, String> {
        panic!("query test uses native store only")
    }
}

fn call(arguments: Value) -> Call {
    Call {
        response: "test".into(),
        mark: None,
        item: json!({}),
        call: "query_test".into(),
        name: "query".into(),
        raw: arguments.to_string(),
        arguments,
    }
}

#[tokio::test]
async fn discovers() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("estate.sqlite");
    santi_estate::Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let service = Service::open(
        crate::service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(Unused),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    for index in 0..7 {
        let id = format!("message_{index}");
        service
            .store
            .place(santi_estate::MessageDraft {
                tag: &id,
                strand: &strand.id,
                actor: message::Role::System,
                actor_id: crate::SYSTEM,
                kind: message::Kind::Text,
                content: &message::Content::text("记忆".repeat(2000)),
                state: message::State::Fixed,
                request: false,
                created: &crate::now(),
            })
            .await
            .unwrap();
        if index < 5 {
            service
                .store
                .create_compact(santi_estate::CompactDraft {
                    tag: &format!("compact_{index}"),
                    strand: &strand.id,
                    first: &id,
                    last: &id,
                    summary: &format!("Memory {index}; live Job job_kept"),
                    metadata: None,
                    expected: None,
                    created: &crate::now(),
                })
                .await
                .unwrap();
        }
    }
    let slots = service.queried(&strand.id, &call(json!({}))).await.unwrap();
    assert_eq!(slots["slots"].as_array().unwrap().len(), 5);
    assert_eq!(slots["default"]["first"], "message_5");
    let first = slots["slots"][0]["id"].as_str().unwrap();
    let text = service
        .queried(&strand.id, &call(json!({"kind":"content", "id":first})))
        .await
        .unwrap();
    assert!(text["text"].as_str().unwrap().contains("job_kept"));
    let policy = service.regimen();
    let limits = santi_estate::Limits {
        slots: policy.slots,
        settled: policy.settled,
    };
    let request = crate::compact::Exec {
        first: None,
        last: None,
        from: None,
        to: None,
        summary: "Memory 0 and 1; live Job job_kept".into(),
        absorb: slots["slots"].as_array().unwrap()[..2]
            .iter()
            .map(|slot| slot["id"].as_str().unwrap().into())
            .collect(),
        capsule: None,
        dry: false,
    };
    service
        .store
        .selection(&strand.id, &request, limits)
        .await
        .unwrap();
    service.exec(&strand.id, request).await.unwrap();
    let slots = service.queried(&strand.id, &call(json!({}))).await.unwrap();
    assert_eq!(slots["slots"].as_array().unwrap().len(), 4);
    let headers = service
        .queried(&strand.id, &call(json!({"kind":"messages"})))
        .await
        .unwrap();
    assert_eq!(headers["messages"][0]["id"], "message_0");
    let piece = service
        .queried(
            &strand.id,
            &call(json!({"kind":"content", "id":"message_0"})),
        )
        .await
        .unwrap();
    assert!(piece["text"].as_str().unwrap().len() <= 8192);
    let next = piece["next"].as_u64().unwrap();
    let rest = service
        .queried(
            &strand.id,
            &call(json!({"kind":"content", "id":"message_0", "offset":next})),
        )
        .await
        .unwrap();
    assert_eq!(
        piece["text"].as_str().unwrap().len() + rest["text"].as_str().unwrap().len(),
        12000
    );
    let other = service.weave().await.unwrap().strand;
    assert!(
        service
            .queried(
                &other.id,
                &call(json!({"kind":"content", "id":"message_0"}))
            )
            .await
            .is_err()
    );
    assert!(
        service
            .queried(&other.id, &call(json!({"kind":"content", "id":first})))
            .await
            .is_err()
    );
    assert!(
        service
            .queried(&strand.id, &call(json!({"kind":"content", "id":"missing"})))
            .await
            .unwrap_err()
            .contains("query slots or messages")
    );
    let mut request = santi_provider::Request {
        model: "unused".into(),
        instructions: None,
        input: Vec::new(),
        tools: None,
        previous: None,
    };
    service.suffix(&mut request);
    let item = request.input.last().unwrap();
    let santi_provider::Item::Message { content, .. } = item else {
        panic!("suffix missing")
    };
    let used = crate::context::budget::gauged(&request).total;
    assert!(content.contains(&format!("used_bytes: {used:020}")));
    assert!(content.contains(&format!("remaining_bytes: {:020}", 75000 - used)));
}
