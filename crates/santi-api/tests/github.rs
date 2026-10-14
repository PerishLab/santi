use axum::http::{HeaderMap, HeaderValue};
use santi_api::webhook::{NormalizedEvent, WebhookAdaptor, WebhookOutcome, github::GithubAdaptor};
use serde_json::{Value, json};

fn received(event: &str, mut payload: Value) -> NormalizedEvent {
    payload["issue"]
        .as_object_mut()
        .unwrap()
        .entry("assignees")
        .or_insert(json!([{"login":"AdamSanti"}]));
    if payload.get("repository").is_none() {
        payload["repository"] = json!({"full_name":"PerishLab/santi"});
    }
    let mut headers = HeaderMap::new();
    headers.insert("X-GitHub-Event", HeaderValue::from_str(event).unwrap());
    let outcome = GithubAdaptor::configured(None, None)
        .addressed(Some("AdamSanti"))
        .normalize(
            &headers,
            &serde_json::to_vec(&payload).unwrap(),
            "secret",
            "ops",
        )
        .unwrap();
    match outcome {
        WebhookOutcome::Event(event) => event,
        _ => panic!("expected event"),
    }
}

#[test]
fn sources() {
    let payload = json!({
        "action": "created",
        "repository": {"full_name": "PerishLab/santi"},
        "issue": {"number": 96, "title": "Incoming work", "body": "issue work",
            "html_url": "https://github.com/PerishLab/santi/issues/96",
            "updated_at": "2026-10-12T08:00:26Z"},
        "comment": {"body": "follow-up work",
            "html_url": "https://github.com/PerishLab/santi/issues/96#issuecomment-1",
            "updated_at": "2026-10-12T08:02:00Z"}
    });
    let issue = received("issues", payload.clone());
    let comment = received("issue_comment", payload);
    assert_eq!(issue.label, comment.label);
    assert!(issue.text.contains("body:\nissue work"));
    assert!(comment.text.contains("body:\nfollow-up work"));
    assert!(!comment.text.contains("body:\nissue work"));
    assert!(comment.text.contains("Updated: 2026-10-12T08:02:00Z"));
    assert!(comment.text.contains("#issuecomment-1"));
    assert_eq!(
        comment.metadata.as_ref().unwrap()["content"]["truncated"],
        false
    );
}

#[test]
fn bounds() {
    let body = "界".repeat(4000);
    let event = received(
        "issues",
        json!({
            "issue": {"number": 96, "title": "题".repeat(1000), "body": body}
        }),
    );
    let text = event.text;
    assert!(text.contains(&"界".repeat(2730)));
    assert!(text.len() < 9500);
    let content = &event.metadata.as_ref().unwrap()["content"];
    assert_eq!(content["bytes"], 12000);
    assert_eq!(content["visible_bytes"], 8190);
    assert_eq!(content["truncated"], true);
    assert_eq!(content["title_truncated"], true);
    assert!(text.contains("Body truncated"));
    assert!(text.contains("Title truncated"));
}

#[test]
fn absent() {
    let event = received("issues", json!({"issue": {"number": 96, "body": null}}));
    let content = &event.metadata.as_ref().unwrap()["content"];
    assert_eq!(content["bytes"], 0);
    assert_eq!(content["available"], false);
    assert!(event.text.contains("Body unavailable"));
}

#[path = "github/scope.rs"]
mod scope;
