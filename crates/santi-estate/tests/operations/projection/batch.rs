use super::support::*;
use serde_json::to_value;

#[tokio::test]
async fn batches() {
    let seat = Seat::new().await;
    let mut calls = Vec::new();
    let mut replies = Vec::new();
    for index in 0..501 {
        seat.pair(index).await;
        calls.push(
            seat.store
                .call(&format!("call_{:04}", 1000 - index))
                .await
                .unwrap()
                .unwrap(),
        );
        replies.push(
            seat.store
                .reply(&format!("result_{:04}", 1000 - index))
                .await
                .unwrap()
                .unwrap(),
        );
    }
    let call = seat.key("ToolCall", "call_1000").await;
    let result = seat.key("ToolResult", "result_1000").await;
    seat.entry("tool_call", call, 1003).await;
    seat.entry("tool_result", result, 1004).await;
    calls.push(calls[0].clone());
    replies.push(replies[0].clone());
    assert_eq!(
        to_value(seat.store.calls("strand_test").await.unwrap()).unwrap(),
        to_value(calls).unwrap()
    );
    assert_eq!(
        to_value(seat.store.results("strand_test").await.unwrap()).unwrap(),
        to_value(replies).unwrap()
    );
    assert!(
        seat.store
            .calls("missing")
            .await
            .unwrap_err()
            .contains("strand not found")
    );
    assert!(
        seat.store
            .results("missing")
            .await
            .unwrap_err()
            .contains("strand not found")
    );
}

#[tokio::test]
async fn empty() {
    let seat = Seat::new().await;
    assert!(seat.store.calls("strand_test").await.unwrap().is_empty());
    assert!(seat.store.results("strand_test").await.unwrap().is_empty());
}
