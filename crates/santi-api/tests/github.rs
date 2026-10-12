use axum::http::{HeaderMap, HeaderValue};
use santi_api::webhook::{NormalizedEvent, WebhookAdaptor, WebhookOutcome, github::GithubAdaptor};
use serde_json::{Value, json};

fn received(event: &str, payload: Value) -> NormalizedEvent {
    let mut headers = HeaderMap::new();
    headers.insert("X-GitHub-Event", HeaderValue::from_str(event).unwrap());
    let outcome = GithubAdaptor::configured(None, None)
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
    assert!(issue.santi_system_text.contains("body:\nissue work"));
    assert!(comment.santi_system_text.contains("body:\nfollow-up work"));
    assert!(!comment.santi_system_text.contains("body:\nissue work"));
    assert!(
        comment
            .santi_system_text
            .contains("updated: 2026-10-12T08:02:00Z")
    );
    assert!(comment.santi_system_text.contains("#issuecomment-1"));
    assert!(comment.santi_system_text.contains("truncated: false"));
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
    let text = event.santi_system_text;
    let (_, visible) = text.split_once("body:\n").unwrap();
    assert!(visible.len() <= 8192);
    assert_eq!(visible, "界".repeat(2730));
    assert!(text.contains("body_bytes: 12000"));
    assert!(text.contains("visible_bytes: 8190"));
    assert!(text.contains("truncated: true"));
    assert!(text.contains("title_truncated: true"));
    assert!(text.len() < 9000);
}

#[test]
fn absent() {
    let event = received("issues", json!({"issue": {"number": 96, "body": null}}));
    assert!(event.santi_system_text.contains("updated: unknown"));
    assert!(event.santi_system_text.contains("body_bytes: 0"));
    assert!(event.santi_system_text.contains("body_available: false"));
    assert!(event.santi_system_text.ends_with("body:\n"));
}
