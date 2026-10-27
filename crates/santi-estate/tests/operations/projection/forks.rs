use super::support::*;
use santi_estate::{ForkDraft, StrandDraft};
use serde_json::to_value;

#[tokio::test]
async fn visibility() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.store.complete_turn("turn_test", 2, NOW).await.unwrap();
    seat.store
        .fork(ForkDraft {
            tag: "strand_child",
            parent: "strand_test",
            at: 2,
            created: NOW,
        })
        .await
        .unwrap();
    let calls = to_value(seat.store.calls("strand_test").await.unwrap()).unwrap();
    let replies = to_value(seat.store.results("strand_test").await.unwrap()).unwrap();
    assert_eq!(
        to_value(seat.store.calls("strand_child").await.unwrap()).unwrap(),
        calls
    );
    assert_eq!(
        to_value(seat.store.results("strand_child").await.unwrap()).unwrap(),
        replies
    );
    seat.store
        .create_strand(StrandDraft {
            tag: "strand_other",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            created: NOW,
        })
        .await
        .unwrap();
    assert!(seat.store.calls("strand_other").await.unwrap().is_empty());
    assert!(seat.store.results("strand_other").await.unwrap().is_empty());
    assert!(
        seat.store
            .end_strand("strand_test")
            .await
            .unwrap_err()
            .contains("strand_child")
    );
    seat.store.end_strand("strand_child").await.unwrap();
    assert_eq!(
        to_value(seat.store.calls("strand_test").await.unwrap()).unwrap(),
        calls
    );
    assert_eq!(
        to_value(seat.store.results("strand_test").await.unwrap()).unwrap(),
        replies
    );
    seat.store.end_strand("strand_test").await.unwrap();
    assert_eq!(
        seat.store.calls("strand_test").await.unwrap_err(),
        "strand not found"
    );
    assert!(seat.store.call("call_1000").await.unwrap().is_none());
    assert!(seat.store.reply("result_1000").await.unwrap().is_none());
    assert!(seat.store.results("strand_other").await.unwrap().is_empty());
}
