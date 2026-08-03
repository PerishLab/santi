use santi::watch::{json_field, next_sse_frame, parse_sse_frame, render_watch_event, snippet};

#[test]
fn renders() {
    assert_eq!(
        render_watch_event("open", r#"{"payload":{"type":"open"}}"#),
        None
    );
    assert_eq!(
        render_watch_event(
            "message",
            r#"{"payload":{"type":"message","beat":"delta","text":"chunk"}}"#,
        ),
        None
    );
    assert_eq!(
        render_watch_event(
            "turn",
            r#"{"payload":{"type":"turn","beat":"started","turn":{"id":"turn_1","trigger":"strand_send"}}}"#,
        )
        .as_deref(),
        Some("turn started turn_1 (strand_send)")
    );
    assert_eq!(
        render_watch_event(
            "turn",
            r#"{"payload":{"type":"turn","beat":"active","activity":{"turn":"turn_1","state":"running_tool"}}}"#,
        )
        .as_deref(),
        Some("turn turn_1: running_tool")
    );
    assert_eq!(
        render_watch_event(
            "message",
            r#"{"payload":{"type":"message","beat":"completed","turn":"turn_1","message":{"text":"hello\nworld"}}}"#,
        )
        .as_deref(),
        Some("assistant completed turn_1: hello world")
    );
    assert_eq!(
        render_watch_event(
            "tool",
            r#"{"payload":{"type":"tool","beat":"replied","result":{"call":"call_1","error":null}}}"#,
        )
        .as_deref(),
        Some("tool result call_1: ok")
    );
    assert_eq!(
        render_watch_event(
            "turn",
            r#"{"payload":{"type":"turn","beat":"failed","turn":"turn_1","error":{"code":"provider.turn.failed","message":"provider request failed","incident":"inc_1"}}}"#,
        )
        .as_deref(),
        Some(
            "turn failed turn_1: provider.turn.failed: provider request failed (incident inc_1)"
        )
    );
    assert_eq!(
        render_watch_event(
            "transition",
            r#"{"payload":{"type":"transition","transition":{"kind":"opened","held":{"id":"inc_1","code":"provider.turn.failed","context":{"detail":"secret"}}}}}"#,
        )
        .as_deref(),
        Some("error opened provider.turn.failed (inc_1)")
    );
}

#[test]
fn snippets() {
    assert_eq!(snippet("a\n  b\t c", 20), "a b c");
    assert_eq!(snippet("abcdef", 3), "abc…");
    assert_eq!(snippet("a\u{1b}b", 20), "a b");
}

#[test]
fn parses() {
    let frame =
        "id: e1\nevent: turn\ndata: {\"payload\":{\"beat\":\"completed\",\"turn\":\"t1\"}}\n";
    let (event, data) = parse_sse_frame(frame).expect("frame");
    assert_eq!(event, "turn");
    assert_eq!(
        data,
        "{\"payload\":{\"beat\":\"completed\",\"turn\":\"t1\"}}"
    );
    assert!(parse_sse_frame(": keep-alive\n").is_none());
}

#[test]
fn reads() {
    let data = "{\"payload\":{\"turn\":{\"id\":\"t9\"}}}";
    assert_eq!(
        json_field(data, &["payload", "turn", "id"]).as_deref(),
        Some("t9")
    );
    assert_eq!(json_field(data, &["payload", "missing"]), None);
}

#[tokio::test]
async fn chunks() {
    use futures_util::stream;

    let chunks: Vec<reqwest::Result<Vec<u8>>> = vec![
        Ok(b"event: turn\ndata: {\"payl".to_vec()),
        Ok(b"oad\":{\"turn\":{\"id\":\"t1\"}}}\n\n: ka\n\n".to_vec()),
    ];
    let mut stream = stream::iter(chunks);
    let mut buffer = Vec::new();
    let (event, data) = next_sse_frame(&mut stream, &mut buffer)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event, "turn");
    assert_eq!(
        json_field(&data, &["payload", "turn", "id"]).as_deref(),
        Some("t1")
    );
    assert!(
        next_sse_frame(&mut stream, &mut buffer)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn unicode() {
    use futures_util::stream;

    let frame = "event: message\ndata: {\"payload\":{\"text\":\"雪\"}}\n\n".as_bytes();
    let split = frame
        .windows(3)
        .position(|held| held == "雪".as_bytes())
        .unwrap()
        + 1;
    let chunks: Vec<reqwest::Result<Vec<u8>>> =
        vec![Ok(frame[..split].to_vec()), Ok(frame[split..].to_vec())];
    let mut stream = stream::iter(chunks);
    let mut buffer = Vec::new();
    let (_, data) = next_sse_frame(&mut stream, &mut buffer)
        .await
        .expect("split UTF-8 frame")
        .expect("message frame");
    assert_eq!(
        json_field(&data, &["payload", "text"]).as_deref(),
        Some("雪")
    );
}

#[tokio::test]
async fn invalid() {
    use futures_util::stream;

    let chunks: Vec<reqwest::Result<Vec<u8>>> =
        vec![Ok(b"event: message\ndata: \xff\n\n".to_vec())];
    let error = next_sse_frame(&mut stream::iter(chunks), &mut Vec::new())
        .await
        .expect_err("invalid UTF-8 must fail");
    assert!(error.to_string().contains("UTF-8"));

    let chunks: Vec<reqwest::Result<Vec<u8>>> = vec![Ok(b"event: turn\ndata: {}".to_vec())];
    let error = next_sse_frame(&mut stream::iter(chunks), &mut Vec::new())
        .await
        .expect_err("incomplete frame must fail");
    assert!(error.to_string().contains("incomplete SSE frame"));

    let chunks: Vec<reqwest::Result<Vec<u8>>> = vec![Ok(b"not-an-sse-field\n\n".to_vec())];
    let error = next_sse_frame(&mut stream::iter(chunks), &mut Vec::new())
        .await
        .expect_err("invalid frame must fail");
    assert!(error.to_string().contains("invalid SSE frame"));
}
