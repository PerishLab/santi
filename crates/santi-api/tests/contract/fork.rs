use santi_core::strand;
use serde_json::{Value, json};

#[test]
fn schema() {
    let text = santi_api::openapi().expect("schema");
    let doc: Value = serde_json::from_str(&text).expect("document");
    let request = &doc["components"]["schemas"]["strand.Fork"];
    assert_eq!(request["properties"]["at"]["minimum"], 0);
    assert!(request["properties"].get("inference").is_some());
    assert_eq!(request["additionalProperties"], false);
    assert!(request["required"].as_array().is_none_or(Vec::is_empty));
    let route = &doc["paths"]["/api/v1/strands/{strand}/fork"]["post"];
    assert!(!route["requestBody"]["required"].as_bool().unwrap_or(false));
    assert!(
        route["requestBody"]["content"]["application/json"]["schema"]
            .to_string()
            .contains("#/components/schemas/strand.Fork")
    );
    let response = &doc["components"]["schemas"]["strand.Forked"];
    assert_eq!(
        response["properties"]["workspace"]["$ref"],
        "#/components/schemas/strand.Workspace"
    );
    assert_eq!(
        doc["components"]["schemas"]["strand.Workspace"]["enum"],
        json!(["current"])
    );
}

#[test]
fn requests() {
    for body in [json!({}), json!({"at": null}), json!({"inference": null})] {
        let draft: strand::Fork = serde_json::from_value(body).expect("default fork");
        assert_eq!(draft.at, None);
        assert_eq!(draft.inference, None);
    }
    let draft: strand::Fork = serde_json::from_value(json!({"at": 3, "inference": {}}))
        .expect("positioned inherited fork");
    assert_eq!(draft.at, Some(3));
    assert!(draft.inference.expect("explicit inheritance").empty());
    for invalid in [
        json!({"at": "3"}),
        json!({"at": 1.5}),
        json!({"position": 3}),
    ] {
        assert!(serde_json::from_value::<strand::Fork>(invalid).is_err());
    }
    assert_eq!(
        serde_json::to_value(strand::Workspace::Current).expect("workspace"),
        "current"
    );
}
