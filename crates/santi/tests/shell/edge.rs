#[allow(dead_code)]
#[path = "../../src/auth.rs"]
mod auth;

use auth::{Credentials, resolve_edge_bearer};

#[tokio::test]
async fn sentinels() {
    let bearer = resolve_edge_bearer(Credentials {
        endpoint: Some(""),
        identity: Some(""),
        username: Some(""),
        password: Some(""),
        key: Some("resolved bearer"),
    })
    .await
    .expect("empty sentinels must not trigger edge auth");

    assert_eq!(bearer.as_deref(), Some("resolved bearer"));
}

#[path = "../../src/client/send/outcome.rs"]
mod outcome;

#[allow(dead_code)]
#[path = "../../src/client/send/fault.rs"]
mod fault;

use fault::{accepted_warning_error, uncertain, unknown};
use outcome::Unsettled;

#[test]
fn typed() {
    let warning = serde_json::json!({
        "code": "runtime.strand.drive_failed",
        "context": { "recovery": { "command": "santi strand drive ss_direct" } }
    });
    let errors = [
        unknown("ss_direct", "POST response was ambiguous".to_string()),
        uncertain(
            "ss_direct",
            "inbox_direct",
            "receipt was unreadable".to_string(),
        ),
        accepted_warning_error(&warning, "inbox_direct"),
    ];

    for error in errors {
        assert!(error.downcast_ref::<Unsettled>().is_some());
        assert!(error.to_string().contains("do not resend"));
    }
}
