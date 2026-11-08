use super::*;

#[tokio::test]
async fn partial_message() {
    let held = run(
        vec![vec![
            received(),
            started("resp_1"),
            Ok(Event::Text("partial output".to_string())),
            reported("length"),
            completed("resp_1"),
        ]],
        None,
    )
    .await;
    assert_eq!(held.status, turn::Status::Completed);
    assert_eq!(held.rounds, vec![Some("length".to_string())]);
    assert_eq!(
        held.finish,
        Some(Some("length".to_string())),
        "terminal finish"
    );
    assert_eq!(held.replies, 1, "partial text reply is preserved");
    assert_eq!(held.texts, vec!["partial output".to_string()]);
    assert_eq!(held.requests, 1);
}

#[tokio::test]
async fn partial_recorded() {
    let held = run(
        vec![vec![
            received(),
            started("resp_1"),
            reported("length"),
            completed("resp_1"),
        ]],
        None,
    )
    .await;
    assert_eq!(held.rounds, vec![Some("length".to_string())]);
    assert_eq!(
        held.finish,
        Some(Some("length".to_string())),
        "terminal finish"
    );
    assert_eq!(held.replies, 0, "empty truncated round yields no reply");
    assert_eq!(held.requests, 1);
    assert!(held.texts.is_empty());
}

#[tokio::test]
async fn request_stop() {
    let held = run(
        vec![vec![
            received(),
            started("resp_1"),
            Ok(Event::Text("finished".to_string())),
            reported("stop"),
            completed("resp_1"),
        ]],
        None,
    )
    .await;
    assert_eq!(held.rounds, vec![Some("stop".to_string())]);
    assert_eq!(
        held.finish,
        Some(Some("stop".to_string())),
        "terminal finish"
    );
    assert_eq!(held.replies, 1);
    assert_eq!(held.requests, 1);
    assert_eq!(held.texts, vec!["finished".to_string()]);
}

#[tokio::test]
async fn output_limit_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let effect = dir.path().join("effect");
    let arguments = json!({"command": format!("printf x >> '{}'", effect.display())});
    let held = run(
        vec![
            vec![
                received(),
                started("resp_1"),
                Ok(Event::Called(Call {
                    response: "resp_1".to_string(),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_1".to_string(),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                reported("length"),
                completed("resp_1"),
            ],
            vec![
                received(),
                started("resp_2"),
                Ok(Event::Text("after call".to_string())),
                reported("stop"),
                completed("resp_2"),
            ],
        ],
        None,
    )
    .await;
    assert_eq!(held.status, turn::Status::Completed);
    assert_eq!(held.requests, 2, "truncated tool round continued once");
    assert_eq!(
        held.finish,
        Some(Some("stop".to_string())),
        "terminal finish reports only the last provider round"
    );
    assert_eq!(held.replies, 1);
    assert_eq!(
        std::fs::read_to_string(&effect).expect("tool effect recorded"),
        "x",
        "tool effect ran exactly once; no replay"
    );
    assert_eq!(held.texts, vec!["after call".to_string()]);
}

#[tokio::test]
async fn completed_turn() {
    let gate = Arc::new(Notify::new());
    let held = run(
        vec![vec![
            received(),
            started("resp_1"),
            Ok(Event::Text("slow".to_string())),
            reported("length"),
            completed("resp_1"),
        ]],
        Some(gate),
    )
    .await;
    assert!(held.running, "provider gate observed a running turn");
    assert_eq!(held.status, turn::Status::Completed);
    assert_eq!(held.finish, Some(Some("length".to_string())));
    assert_eq!((held.replies, held.requests), (1, 1));
    assert_eq!(held.texts, vec!["slow".to_string()]);
}

#[tokio::test]
async fn replay_shape() {
    let arguments = json!({"command": "true"});
    let held = run(
        vec![
            vec![
                received(),
                started("resp_1"),
                Ok(Event::Called(Call {
                    response: "resp_1".to_string(),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_1".to_string(),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                reported("length"),
                completed("resp_1"),
            ],
            vec![
                received(),
                started("resp_2"),
                Ok(Event::Text("no report".to_string())),
                completed("resp_2"),
            ],
        ],
        None,
    )
    .await;
    assert_eq!(held.status, turn::Status::Completed);
    assert_eq!(held.rounds, vec![Some("length".to_string()), None]);
    assert_eq!(
        held.finish,
        Some(None),
        "last round without report stays unknown; no fallback"
    );
    assert_eq!((held.replies, held.requests), (1, 2));
    assert_eq!(held.texts, vec!["no report".to_string()]);
}
