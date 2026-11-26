use super::*;
use santi_api::retire;

#[tokio::test]
async fn retirement() {
    let temp = tempfile::tempdir().expect("temp");
    let database = temp.path().join("santi.sqlite");
    let store = support::bootstrap(&database).await;
    let service = Service::open(
        "fixture/model/execution",
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(Driver),
    )
    .await
    .expect("service");
    let strand = service.weave().await.expect("strand").strand.id;
    let child = service.fork(&strand).await.expect("fork").strand.id;
    let refused = retire(State(service.clone()), Path(strand.clone()))
        .await
        .expect_err("live fork");
    assert_eq!(refused.status(), StatusCode::CONFLICT);
    assert!(refused.message().contains(&child));
    assert_eq!(
        retire(State(service.clone()), Path(child.clone()))
            .await
            .map_err(|e| e.message().to_string())
            .expect("end child"),
        StatusCode::NO_CONTENT
    );
    store
        .accept(
            Arrival {
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
    let refused = retire(State(service.clone()), Path(strand.clone()))
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
    let missing = retire(State(service), Path(child))
        .await
        .expect_err("already ended");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let document: serde_json::Value =
        serde_json::from_str(&santi_api::openapi().expect("openapi")).expect("json");
    let responses = &document["paths"]["/api/v1/strands/{strand}"]["delete"]["responses"];
    for status in ["204", "404", "409", "500"] {
        assert!(responses.get(status).is_some(), "missing response {status}");
    }
}
