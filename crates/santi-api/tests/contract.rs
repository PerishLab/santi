use sha2::{Digest, Sha256};

#[test]
fn api() {
    let text = santi_api::export_openapi_json().expect("export openapi");
    let document: serde_json::Value = serde_json::from_str(&text).expect("parse openapi");
    assert_eq!(document["info"]["version"].as_str(), Some("v1"));
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
    ] {
        assert!(document["components"]["schemas"].get(schema).is_some());
    }
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
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "8aa91cb02001efb73af1fdd4b92c0a18658cdea1c1bc9475e2325c3166dc9a79"
    );
}
