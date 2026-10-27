use super::*;
use crate::client::tui::keys::Stroke;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn waiting() -> State {
    let mut state = State::new("soul_test".into(), "ss_test".into(), Default::default());
    state.typed = "first message".into();
    assert!(matches!(state.struck(Some(Stroke::Enter)), Step::Speak(_)));
    state.heard(Some(Beat::Accepted("inbox_test".into())));
    state.heard(Some(Beat::Unsettled(
        "watch timed out; do not resend".into(),
    )));
    state.typed = "next draft".into();
    state
}

#[tokio::test]
async fn reconciles() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for body in [
            "invalid JSON",
            r#"{"inbox":"inbox_test","strand":"other","state":"completed"}"#,
            r#"{"inbox":"other","strand":"ss_test","state":"completed"}"#,
            r#"{"inbox":"inbox_test","strand":"ss_test","state":"unrecognized"}"#,
            r#"{"inbox":"inbox_test","strand":"ss_test","state":"driving"}"#,
            r#"{"inbox":"inbox_test","strand":"ss_test","state":"completed"}"#,
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let count = stream.read(&mut request).await.unwrap();
            assert!(
                String::from_utf8_lossy(&request[..count])
                    .starts_with("GET /api/v1/receipts/inbox_test ")
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let target = Target::tui(&client, &base, "ss_test");
    let mut state = waiting();
    for _ in 0..5 {
        assert_eq!(
            apply(&mut state, inspect(target, "inbox_test".into()).await),
            Step::Stay
        );
        assert!(state.busy && state.unsettled);
        assert_eq!(state.typed, "next draft");
    }
    assert_eq!(
        apply(&mut state, inspect(target, "inbox_test".into()).await),
        Step::Refresh
    );
    assert!(!state.busy && !state.unsettled && state.since.is_none());
    assert!(state.receipt.is_none());
    assert_eq!(state.typed, "next draft");
    assert!(matches!(state.struck(Some(Stroke::Enter)), Step::Speak(_)));
    server.await.unwrap();
}

#[test]
fn failed() {
    let mut state = waiting();
    assert_eq!(
        apply(&mut state, ("old_receipt".into(), Some(Proof::Completed))),
        Step::Stay
    );
    assert!(state.busy && state.unsettled);
    state.typed = "/status".into();
    assert_eq!(state.struck(Some(Stroke::Enter)), Step::Refresh);
    assert!(state.busy && state.unsettled);
    assert_eq!(
        apply(&mut state, ("inbox_test".into(), Some(Proof::Failed))),
        Step::Refresh
    );
    assert!(!state.busy && !state.unsettled);
    assert!(state.receipt.is_none());
}
