use super::*;
use santi_api::end_strand_handler;

#[tokio::test]
async fn retirement() {
    let temp = tempfile::tempdir().expect("temp");
    let database = temp.path().join("santi.sqlite");
    let store = support::bootstrap(&database).await;
    let service = Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
    )
    .await
    .expect("service");
    let strand = service.weave().await.expect("strand").strand.id;
    let child = service.fork(&strand).await.expect("fork").strand.id;
    let refused = end_strand_handler(State(service.clone()), Path(strand.clone()))
        .await
        .expect_err("live fork");
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(refused.message().contains(&child));
    assert_eq!(
        end_strand_handler(State(service.clone()), Path(child.clone()))
            .await
            .map_err(|e| e.message().to_string())
            .expect("end child"),
        StatusCode::NO_CONTENT
    );
    store
        .accept_inbox(
            InboxDraft {
                tag: "retirement_input",
                strand: &strand,
                kind: message::Kind::Text,
                content: &message::Content::text("keep"),
                source: None,
                created: &santi_core::now(),
            },
            10,
        )
        .await
        .expect("accepted");
    let refused = end_strand_handler(State(service.clone()), Path(strand.clone()))
        .await
        .expect_err("pending input");
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(
        store
            .receipt("retirement_input")
            .await
            .expect("receipt")
            .is_some()
    );
    let missing = end_strand_handler(State(service), Path(child))
        .await
        .expect_err("already ended");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let document: serde_json::Value =
        serde_json::from_str(&santi_api::export_openapi_json().expect("openapi")).expect("json");
    let responses = &document["paths"]["/api/v1/strands/{strand}"]["delete"]["responses"];
    for status in ["204", "404", "409", "500"] {
        assert!(responses.get(status).is_some(), "missing response {status}");
    }
}
