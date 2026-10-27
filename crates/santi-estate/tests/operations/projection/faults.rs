use super::support::*;

#[tokio::test]
async fn outcomes() {
    let seat = Seat::new().await;
    let call = seat
        .store
        .create_call(santi_estate::CallDraft {
            tag: "call_empty",
            turn: "turn_test",
            tool: "shell",
            arguments: &serde_json::json!({}),
            created: NOW,
        })
        .await
        .unwrap();
    let call = seat.key("ToolCall", &call.id).await.to_string();
    let result = seat
        .core
        .put(
            "ToolResult",
            &[("tag", "result_empty"), ("call", &call), ("created", NOW)],
        )
        .await
        .unwrap();
    seat.entry("tool_result", result, 2).await;
    assert_eq!(
        seat.store.results("strand_test").await.unwrap_err(),
        "tool result has no outcome"
    );
    seat.core
        .put(
            "ToolOutput",
            &[("result", &result.to_string()), ("output", "{}")],
        )
        .await
        .unwrap();
    seat.core
        .put(
            "ToolFailure",
            &[("result", &result.to_string()), ("error", "broken")],
        )
        .await
        .unwrap();
    assert_eq!(
        seat.store.results("strand_test").await.unwrap_err(),
        "tool result has conflicting outcomes"
    );
}

#[tokio::test]
async fn malformed() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.damage("UPDATE turn_toolcall SET arguments = '{'")
        .await;
    assert_eq!(
        seat.store.calls("strand_test").await.unwrap_err(),
        seat.store.call("call_1000").await.unwrap_err()
    );
    seat.damage("UPDATE toolresult_tooloutput SET output = '{'")
        .await;
    assert_eq!(
        seat.store.results("strand_test").await.unwrap_err(),
        seat.store.reply("result_1000").await.unwrap_err()
    );
}

#[tokio::test]
async fn targets() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.damage("UPDATE strand_strandentry SET call_id = 900000 WHERE target_type = 'tool_call'")
        .await;
    assert_eq!(
        seat.store.calls("strand_test").await.unwrap_err(),
        "ToolCall 900000 missing"
    );
    seat.damage(
        "UPDATE strand_strandentry SET result_id = 900000 WHERE target_type = 'tool_result'",
    )
    .await;
    assert_eq!(
        seat.store.results("strand_test").await.unwrap_err(),
        "ToolResult 900000 missing"
    );
}

#[tokio::test]
async fn relations() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.damage("UPDATE turn_toolcall SET turn_id = 900000")
        .await;
    assert_eq!(
        seat.store.calls("strand_test").await.unwrap_err(),
        "Turn 900000 missing"
    );
    seat.damage("UPDATE toolcall_toolresult SET call_id = 900000")
        .await;
    assert_eq!(
        seat.store.results("strand_test").await.unwrap_err(),
        "ToolCall 900000 missing"
    );
}
