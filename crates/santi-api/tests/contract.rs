use sha2::{Digest, Sha256};

#[test]
fn api() {
    let text = santi_api::export_openapi_json().expect("export openapi");
    let document: serde_json::Value = serde_json::from_str(&text).expect("parse openapi");
    assert_eq!(document["info"]["version"].as_str(), Some("v1"));
    let request = &document["components"]["schemas"]["CreateJobRequest"];
    assert!(request["properties"].get("notify_on_terminal").is_some());
    assert!(
        !request["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "notify_on_terminal")
    );
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
        "turn.activity.Current",
        "turn.activity.Snapshot",
        "turn.activity.Phase",
        "stream.ExecutionOrder",
        "stream.ExecutionRecord",
        "stream.ExecutionTail",
        "thinking.Round",
        "thinking.Continuation",
        "thinking.Usage",
    ] {
        assert!(document["components"]["schemas"].get(schema).is_some());
    }
    let turn = &document["components"]["schemas"]["turn.Turn"];
    let reference = turn["properties"]["finish"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|shape| shape["$ref"].as_str());
    assert_eq!(reference, Some("#/components/schemas/turn.Finish"));
    let finish = &document["components"]["schemas"]["turn.Finish"];
    assert!(
        finish["description"]
            .as_str()
            .unwrap()
            .contains("never states why the turn ended")
    );
    assert!(
        finish["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "span")
    );
    for field in ["span", "response", "status", "incomplete"] {
        assert!(finish["properties"].get(field).is_some());
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
    let snapshot = &document["components"]["schemas"]["budget.Snapshot"];
    assert!(snapshot["properties"].get("request").is_some());
    assert!(snapshot["properties"].get("pressure").is_some());
    assert!(snapshot["properties"].get("estimate").is_none());
    let request = &document["components"]["schemas"]["budget.Request"];
    assert!(request["properties"].get("basis").is_some());
    assert!(request["properties"].get("pending").is_some());
    assert!(request["properties"].get("ceiling").is_some());
    let pressure = &document["components"]["schemas"]["budget.Pressure"];
    assert!(pressure["properties"].get("active_bytes").is_some());
    assert!(pressure["properties"].get("active_ceiling_bytes").is_some());
    let slots = &document["components"]["schemas"]["budget.Slots"];
    assert!(slots["properties"].get("settled_bytes").is_some());
    assert!(slots["properties"].get("settled_ceiling_bytes").is_some());
    assert!(slots["properties"].get("active").is_none());
    assert!(slots["properties"].get("ceiling").is_none());
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
    let acknowledgement = &routes["/api/v1/receipts/{inbox}/acknowledgement"]["get"];
    assert_eq!(
        acknowledgement["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/receipt.Acknowledgement"
    );
    assert!(
        acknowledgement["summary"]
            .as_str()
            .unwrap()
            .contains("uncertain")
    );
    let schema = &document["components"]["schemas"]["receipt.Acknowledgement"];
    for field in ["inbox", "intent", "state", "created", "updated"] {
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == field)
        );
    }
    let confirmation = &routes["/api/v1/receipts/{inbox}/acknowledgement/confirm"]["post"];
    assert!(
        confirmation["summary"]
            .as_str()
            .unwrap()
            .contains("attestation")
    );
    assert!(schema["properties"].get("resolution").is_some());
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "12b67bf8479cf17c25e66febe6a810614cfacc2fe070ac4b3cf67777928c0027"
    );
}
