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
    let timeout = &document["components"]["schemas"]["budget.Execution"]["properties"]["timeout"];
    assert_eq!(timeout["default"], 60);
    assert_eq!(timeout["minimum"], 1);
    assert_eq!(timeout["maximum"], 3600);
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
    let rendering = &document["components"]["schemas"]["render.Kind"];
    assert_eq!(
        rendering["enum"],
        serde_json::json!(["system_prompt", "skill"])
    );
    assert!(
        rendering["description"]
            .as_str()
            .unwrap()
            .contains("does not change material bindings")
    );
    assert!(
        document["components"]["schemas"]
            .get("material.Material")
            .is_none()
    );
    assert!(
        document["components"]["schemas"]
            .get("material.Kind")
            .is_none()
    );
    assert!(
        document["components"]["schemas"]
            .get("material.Descriptor")
            .is_some()
    );
    let routes = &document["paths"];
    assert!(
        routes["/api/v1/strands/{strand}/materials"]["post"]["summary"]
            .as_str()
            .unwrap()
            .contains("Legacy render adapter")
    );
    for path in ["/api/v1/errors/events", "/api/v1/turn-events/stream"] {
        assert!(
            routes[path]["get"]["responses"]["200"]["content"]
                .get("text/event-stream")
                .is_some()
        );
        assert!(
            routes[path]["get"]["summary"]
                .as_str()
                .unwrap()
                .contains("observation time")
        );
    }
    for (method, path, header) in [
        ("post", "/api/v1/jobs", "x-santi-job-capability"),
        ("get", "/api/v1/jobs", "x-santi-soul-id"),
        ("get", "/api/v1/jobs/{job}/logs", "x-santi-soul-id"),
    ] {
        assert!(
            routes[path][method]["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value["in"] == "header"
                    && value["name"] == header
                    && value["required"] == true)
        );
    }
    for path in [
        "/api/v1/materials",
        "/api/v1/souls/{soul}/materials",
        "/api/v1/strands/{strand}/materials",
    ] {
        assert!(routes[path].get("get").is_some());
    }
    assert!(
        document["components"]["schemas"]
            .get("material.Descriptor")
            .is_some()
    );
    assert!(
        document["components"]["schemas"]["material.Descriptor"]["properties"]
            .get("revision")
            .is_some()
    );
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "640c063a081f2fa5f7ecc5c6d7d6e4e05e3d8fb5c39e761db46cb388af306bf7"
    );
}
