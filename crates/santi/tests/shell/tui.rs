#[path = "tui/server.rs"]
mod server;

use santi::cli::ClientDefaults;
use santi::client::tui::{Request, session};
use server::spawn_server;
use std::io::Cursor;

#[tokio::test]
async fn starts() {
    let server = spawn_server().await;
    let temp = tempfile::tempdir().expect("temp dir");
    let memory = temp.path().join("jarvis.md");
    std::fs::write(&memory, "You are Jarvis.").expect("write memory");
    let defaults = ClientDefaults {
        soul: None,
        strand: None,
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("hello\n/reload\n/exit\n");
    let mut output = Vec::new();
    session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            bearer: None,
            memory: Some(memory.display().to_string()),
        },
        &mut input,
        &mut output,
    )
    .await
    .expect("cold start");
    let output = String::from_utf8(output).expect("utf8 output");
    assert!(output.contains("soul: soul_jarvis"));
    assert!(output.contains("strand: ss_jarvis"));
    assert!(output.contains("context: 1200/8000 bytes"));
    assert!(output.contains("reload refused: /reload requires an interactive terminal"));
    assert!(output.contains("soul> I am awake."));
    assert_eq!(output.matches("I am awake.").count(), 1);
    assert!(output.contains("assistant completed turn_missing: Recovered speech."));
    assert!(output.contains("event stream gap: 2 events were not delivered"));
    assert_eq!(output.matches("Gap-safe speech.").count(), 2);
    assert!(output.contains("assistant completed turn_gap: Gap-safe speech."));
    let completed = output
        .find("send completed: receipt inbox_jarvis is durably completed; do not resend")
        .expect("completed receipt proof");
    let auxiliary = output
        .find("auxiliary status refresh failed after completed receipt inbox_jarvis; do not resend")
        .expect("auxiliary refresh guidance");
    assert!(completed < auxiliary);
    assert!(!output.contains('\u{1b}'));
    let marker = output.find("JARVIS_LIVE_PREDISPATCH").expect("live marker");
    let calling = output.find("turn turn_seed: calling").expect("calling");
    let dispatch = output.find("tool call shell").expect("tool dispatch");
    assert!(marker < calling);
    assert!(calling < dispatch);
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 7);
    assert!(requests[0].2.contains(r#""memory":"You are Jarvis.""#));
    let send = requests
        .iter()
        .find(|(method, path, _)| method == "POST" && path == "/api/v1/strands/ss_jarvis/send")
        .expect("send request");
    assert!(send.2.contains(r#""text":"hello""#));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.0 == "POST" && request.1.ends_with("/send"))
            .count(),
        1
    );
}
#[tokio::test]
async fn refuses() {
    let server = spawn_server().await;
    let defaults = ClientDefaults {
        soul: Some("soul_jarvis".to_string()),
        strand: Some("ss_foreign".to_string()),
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("/exit\n");
    let mut output = Vec::new();
    let error = session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            bearer: None,
            memory: None,
        },
        &mut input,
        &mut output,
    )
    .await
    .expect_err("foreign strand must be rejected");
    assert!(error.to_string().contains("belongs to soul soul_other"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/api/v1/strands/ss_foreign");
}
#[tokio::test]
async fn owner() {
    let server = spawn_server().await;
    let defaults = ClientDefaults {
        soul: None,
        strand: Some("ss_ownerless".to_string()),
    };
    let client = reqwest::Client::new();
    let mut input = Cursor::new("/exit\n");
    let mut output = Vec::new();
    let error = session(
        Request {
            client: &client,
            base: &server.base_url,
            defaults: &defaults,
            bearer: None,
            memory: None,
        },
        &mut input,
        &mut output,
    )
    .await
    .expect_err("ownerless strand must be rejected");
    assert!(error.to_string().contains("missing valid /strand/soul"));
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "GET");
    assert_eq!(requests[0].1, "/api/v1/strands/ss_ownerless");
}
