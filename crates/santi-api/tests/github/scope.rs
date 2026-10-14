use super::*;
use santi_api::webhook::WebhookError;

fn payload(action: &str) -> Value {
    json!({
        "action": action,
        "repository": {"full_name":"PerishLab/santi"},
        "sender": {"login":"PerishCode"},
        "issue": {"number":96,"title":"Delivery","body":"Acceptance and references",
            "assignees":[],"html_url":"https://github.com/PerishLab/santi/issues/96"},
        "comment": {"body":"Follow-up","html_url":"https://github.com/PerishLab/santi/issues/96#issuecomment-7"}
    })
}

#[test]
fn assignment() {
    let mut input = payload("assigned");
    input["assignee"] = json!({"login":"adamsanti"});
    input["issue"]["assignees"] = json!([{"login":"AdamSanti"}]);
    let event = received("issues", input.clone());
    assert!(event.in_scope);
    assert!(event.text.contains("@PerishCode assigned you (@AdamSanti)"));
    assert!(event.text.contains("Acceptance and references"));
    assert_eq!(event.metadata.unwrap()["relationship"], "assignment");
    input["action"] = json!("unassigned");
    input["issue"]["assignees"] = json!([]);
    let removed = received("issues", input);
    assert!(removed.in_scope);
    assert_eq!(event.label, removed.label);
    assert!(
        removed
            .text
            .contains("removed you (@AdamSanti) as an Assignee")
    );
    assert_eq!(
        removed.metadata.unwrap()["relationship"],
        "assignment_removed"
    );
}

#[test]
fn responsibility() {
    let mut input = payload("created");
    assert!(!received("issue_comment", input.clone()).in_scope);
    input["issue"]["assignees"] = json!([{"login":"AdamSanti"}]);
    let event = received("issue_comment", input.clone());
    assert!(event.in_scope);
    assert!(
        event
            .text
            .contains("posted a comment on an Issue assigned to you")
    );
    input["action"] = json!("closed");
    let event = received("issues", input);
    assert!(event.in_scope);
    assert!(event.text.contains("closed an Issue assigned to you"));
}

#[test]
fn mentions() {
    for (body, related) in [
        ("@AdamSanti please review", true),
        ("请@adamsanti 看看", true),
        ("@AdamSantiElse", false),
        ("person@AdamSanti.example", false),
        ("`@AdamSanti` example", false),
        ("``@AdamSanti`` example", false),
        ("an unmatched ` before @AdamSanti", true),
        ("```shell\n@AdamSanti\n```", false),
        ("\\@AdamSanti", false),
    ] {
        let mut input = payload("created");
        input["comment"]["body"] = json!(body);
        let event = received("issue_comment", input);
        assert_eq!(event.in_scope, related, "{body}");
        if related {
            assert!(event.text.contains("addressing you (@AdamSanti)"));
            assert_eq!(event.metadata.unwrap()["relationship"], "mention");
        }
    }
    let mut input = payload("opened");
    input["issue"]["body"] = json!("@AdamSanti review this proposal");
    assert!(received("issues", input.clone()).in_scope);
    input["action"] = json!("labeled");
    assert!(!received("issues", input.clone()).in_scope);
    input["action"] = json!("edited");
    input["changes"] = json!({"body":{"from":"previous proposal"}});
    assert!(received("issues", input).in_scope);
}

#[test]
fn edits() {
    let mut input = payload("edited");
    input["changes"] = json!({"body":{"from":"@AdamSanti please review"}});
    assert!(received("issue_comment", input.clone()).in_scope);
    input["action"] = json!("deleted");
    input["comment"]["body"] = json!("@AdamSanti please review");
    assert!(received("issue_comment", input).in_scope);
}

#[test]
fn identity() {
    let headers = HeaderMap::from_iter([(
        "X-GitHub-Event".parse().unwrap(),
        HeaderValue::from_static("issues"),
    )]);
    let mut input = payload("opened");
    let data = serde_json::to_vec(&input).unwrap();
    assert!(matches!(
        GithubAdaptor::configured(None, None).normalize(&headers, &data, "secret", "ops"),
        Err(WebhookError::Unavailable(_))
    ));
    input["issue"].as_object_mut().unwrap().remove("assignees");
    assert!(
        GithubAdaptor::configured(None, None)
            .addressed(Some("AdamSanti"))
            .normalize(
                &headers,
                &serde_json::to_vec(&input).unwrap(),
                "secret",
                "ops"
            )
            .is_err()
    );
    input["sender"]["login"] = json!("AdamSanti");
    let event = received("issues", input);
    assert!(event.self_authored);
    assert!(!event.in_scope);
}
