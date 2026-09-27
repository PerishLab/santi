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
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "4e03432de52193576baf6cc73eb67790fb7243c0f0a0fd97db53e899816163d4"
    );
}
