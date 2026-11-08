use super::server::{Scenario, spawn};
use santi::cli::WatchFormat;
use santi::client::{Request, Target, emit};

async fn rendered(base: &str, format: WatchFormat) -> (anyhow::Result<()>, String) {
    let client = reqwest::Client::new();
    let mut output = Vec::new();
    let result = emit(
        Request {
            target: Target::new(&client, base, "ss_cli", format),
            body: serde_json::json!({"content":[{"type":"text","text":"hello"}]}),
            watch: true,
        },
        &mut output,
    )
    .await;
    (result, String::from_utf8(output).unwrap())
}

#[tokio::test]
async fn filtered() {
    for scenario in [Scenario::Fast, Scenario::Queued] {
        let server = spawn(scenario).await;
        let (result, output) = rendered(&server.base, WatchFormat::Filtered).await;
        result.unwrap();
        let summary = output.lines().last().unwrap();
        assert!(
            summary.starts_with("receipt inbox_cli completed for strand ss_cli;"),
            "{output}"
        );
        assert!(summary.contains("does not establish Strand idleness or business completion"));
        let command = format!("santi --base-url '{}' receipt 'inbox_cli'", server.base);
        assert!(summary.contains(&command), "{output}");
        assert_eq!(output.matches("receipt inbox_cli completed").count(), 1);
        assert_eq!(
            server.counts.post.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}

#[tokio::test]
async fn raw() {
    let server = spawn(Scenario::Fast).await;
    let (result, output) = rendered(&server.base, WatchFormat::Raw).await;
    result.unwrap();
    assert_eq!(
        output,
        "{\"payload\":{\"type\":\"turn\",\"beat\":\"completed\",\"turn\":\"turn_seed\"}}\n"
    );
    for line in output.lines() {
        serde_json::from_str::<serde_json::Value>(line).expect("raw stream stays JSON lines");
    }
}

#[tokio::test]
async fn failure() {
    for scenario in [
        Scenario::Failed,
        Scenario::Closed,
        Scenario::Post,
        Scenario::Broken,
        Scenario::Inbox,
        Scenario::Strand,
    ] {
        let server = spawn(scenario).await;
        let (result, output) = rendered(&server.base, WatchFormat::Filtered).await;
        let error = result.unwrap_err().to_string();
        let command = format!("santi --base-url '{}'", server.base);
        assert!(error.contains(&command), "{error}");
        assert!(error.contains("do not resend"));
        assert!(!output.contains("receipt inbox_cli completed"));
        assert_eq!(
            server.counts.post.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}

#[tokio::test]
async fn subscription() {
    for credentials in [false, true] {
        let server = spawn(Scenario::Status).await;
        let base = if credentials {
            server.base.replace("http://", "http://user:private-token@")
        } else {
            server.base.clone()
        };
        let (result, output) = rendered(&base, WatchFormat::Filtered).await;
        let error = format!("{:#}", result.unwrap_err());
        assert!(!error.contains("private-token"));
        if credentials {
            assert!(error.contains("credential-free"), "{error}");
            assert!(!error.contains("`santi"));
        } else {
            assert!(
                error.contains(&format!("--base-url '{}'", server.base)),
                "{error}"
            );
        }
        assert!(!error.contains("do not resend"));
        assert!(output.is_empty());
        assert_eq!(
            server.counts.post.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
}
