use sha2::{Digest, Sha256};

#[test]
fn api() {
    let text = santi_api::export_openapi_json().expect("export openapi");
    let document: serde_json::Value = serde_json::from_str(&text).expect("parse openapi");
    assert_eq!(document["info"]["version"].as_str(), Some("v1"));
    assert!(
        document["paths"]["/api/v1/strands/{strand}/fork"]["post"]["summary"]
            .as_str()
            .unwrap()
            .contains("in-flight or failed turns")
    );
    let description = document["components"]["schemas"]["compact.Compact"]["description"]
        .as_str()
        .expect("compact description");
    assert!(description.contains("`authoritative` is `false`"));
    assert!(description.contains("`basis` is `precommit_preview`"));
    assert!(description.contains("`context_estimate.forecast`"));
    assert!(description.contains("not exact postcommit context"));
    let execution_tail = &document["paths"]["/api/v1/strands/{strand}/execution-tail"]["get"]["responses"]
        ["200"]["content"]["application/json"]["schema"];
    assert_eq!(
        execution_tail["$ref"].as_str(),
        Some("#/components/schemas/stream.ExecutionTail")
    );
    for schema in [
        "stream.ExecutionOrder",
        "stream.ExecutionRecord",
        "stream.ExecutionTail",
        "thinking.Round",
        "thinking.Continuation",
        "thinking.Usage",
    ] {
        assert!(document["components"]["schemas"].get(schema).is_some());
    }
    let span = &document["components"]["schemas"]["thinking.Span"];
    assert!(span["properties"].get("round").is_some());
    let continuation = &document["components"]["schemas"]["thinking.Continuation"];
    assert!(continuation["properties"].get("content").is_some());
    assert!(
        !continuation["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "content")
    );
    let cap = &document["components"]["schemas"]["budget.Cap"];
    let soul = &document["components"]["schemas"]["soul.Soul"];
    assert!(soul["properties"].get("identity").is_some());
    assert!(
        !soul["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "identity")
    );
    assert!(
        document["components"]["schemas"]
            .get("soul.Identity")
            .is_some()
    );
    assert!(cap["properties"].get("maintenance").is_some());
    assert!(
        !cap["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field == "maintenance")
    );
    assert!(
        document["components"]["schemas"]["budget.Execution"]["properties"]
            .get("lines")
            .is_some()
    );
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "d7b3e705094bdd51f2a6a77f7ea936feacb4c2e18ea05c884636b664a73ec1d9"
    );
}
