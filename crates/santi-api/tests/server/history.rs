use super::*;
use axum::extract::Query;
use santi_api::{ErrorQueryParams, errors_handler, incident_handler, strand_errors_handler};
use santi_core::{Scope, Source, catalog};

#[tokio::test]
async fn history() {
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
    let target = service.weave().await.expect("strand").strand.id;
    let foreign = service.weave().await.expect("other").strand.id;
    let scope = Scope::new("strand", &target);
    let first = raise(&store, &scope).await;
    store
        .resolve(
            &catalog::INTERNAL.key("strand", &target),
            "operator",
            serde_json::json!({}),
            &santi_core::now(),
        )
        .await
        .expect("resolve");
    let second = raise(&store, &scope).await;
    let other = raise(&store, &Scope::new("strand", &foreign)).await;
    let initial = strand_errors_handler(
        State(service.clone()),
        Path(target.clone()),
        Query(query(None)),
    )
    .await
    .map_err(|e| e.message().to_string())
    .expect("list")
    .0;
    assert_eq!(initial.len(), 1);
    let cursor = initial[0].id.clone();
    let page = strand_errors_handler(
        State(service.clone()),
        Path(target.clone()),
        Query(query(Some(cursor.clone()))),
    )
    .await
    .map_err(|e| e.message().to_string())
    .expect("page")
    .0;
    assert_eq!(page.len(), 1);
    assert_ne!(page[0].id, cursor);
    for id in [&first, &second] {
        let held = incident_handler(State(service.clone()), Path(id.clone()))
            .await
            .map_err(|e| e.message().to_string())
            .expect("inspect")
            .0;
        assert_eq!(held.incident.id, *id);
        assert_eq!(held.transitions.len(), if *id == first { 2 } else { 1 });
    }
    let error = strand_errors_handler(
        State(service.clone()),
        Path(target.clone()),
        Query(query(Some(other))),
    )
    .await
    .expect_err("foreign cursor");
    assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    let error = strand_errors_handler(
        State(service.clone()),
        Path("missing".into()),
        Query(query(None)),
    )
    .await
    .expect_err("missing strand");
    assert_eq!(error.status(), StatusCode::NOT_FOUND);
    let empty = errors_handler(
        State(service.clone()),
        Path(("runtime".into(), "default".into())),
        Query(query(None)),
    )
    .await
    .map_err(|e| e.message().to_string())
    .expect("compatible empty list")
    .0;
    assert!(empty.is_empty());
    let missing = incident_handler(State(service), Path("missing".into()))
        .await
        .expect_err("missing incident");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let doc: serde_json::Value =
        serde_json::from_str(&santi_api::export_openapi_json().expect("export")).expect("json");
    assert!(
        doc["paths"]["/api/v1/incidents/{incident}"]["get"]["responses"]
            .get("404")
            .is_some()
    );
    assert!(doc["components"]["schemas"].get("error.History").is_some());
}

fn query(before: Option<String>) -> ErrorQueryParams {
    ErrorQueryParams {
        limit: Some(1),
        before,
    }
}

async fn raise(store: &santi_core::Store, scope: &Scope) -> String {
    store
        .raise(
            santi_core::error::Draft {
                key: catalog::INTERNAL.key(&scope.kind, &scope.id),
                descriptor: catalog::INTERNAL,
                scope: scope.clone(),
                source: Source::new("test", "history"),
                message: "retained".into(),
                context: serde_json::json!({}),
            },
            &santi_core::now(),
        )
        .await
        .expect("raise")
        .incident
        .expect("id")
}
