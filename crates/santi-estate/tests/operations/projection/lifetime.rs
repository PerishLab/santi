use super::support::*;
use santi_estate::{CallDraft, ForkDraft, Store, StrandDraft};
use serde_json::{json, to_value};

#[tokio::test]
async fn shared() {
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
    let parent = seat.store.entries("strand_test").await.unwrap();
    let child = seat.store.entries("strand_child").await.unwrap();
    assert_eq!(child.len(), parent.len());
    for (a, b) in parent.iter().zip(child.iter()) {
        assert_eq!(a.target, b.target);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.seq, b.seq);
        assert_eq!(b.strand, "strand_child");
    }
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
    assert!(seat.store.entries("strand_other").await.unwrap().is_empty());
    assert!(
        seat.store
            .end_strand("strand_test")
            .await
            .unwrap_err()
            .contains("strand_child")
    );
    seat.store.end_strand("strand_child").await.unwrap();
    assert_eq!(
        to_value(seat.store.entries("strand_test").await.unwrap()).unwrap(),
        to_value(parent).unwrap()
    );
    seat.store.end_strand("strand_test").await.unwrap();
    assert_eq!(
        seat.store.entries("strand_test").await.unwrap_err(),
        "strand not found"
    );
    assert!(seat.store.entries("strand_other").await.unwrap().is_empty());
}

#[tokio::test]
async fn appended() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    let reader = Store::open(seat.temp.path().join("estate.sqlite"))
        .await
        .unwrap();
    assert_eq!(reader.entries("strand_test").await.unwrap().len(), 2);
    for index in 0..12 {
        let tag = format!("call_new_{index}");
        let arguments = json!({"index":index});
        let draft = CallDraft {
            tag: &tag,
            turn: "turn_test",
            tool: "shell",
            arguments: &arguments,
            created: NOW,
        };
        let (read, write) =
            tokio::join!(reader.entries("strand_test"), seat.store.create_call(draft));
        write.unwrap();
        if let Ok(entries) = read {
            assert!((2 + index..=3 + index).contains(&entries.len()));
        } else {
            assert_eq!(read.unwrap_err(), "store unavailable");
        }
        let entries = reader.entries("strand_test").await.unwrap();
        assert_eq!(entries.last().unwrap().target, tag);
    }
}

#[tokio::test]
async fn retired() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    seat.store.complete_turn("turn_test", 2, NOW).await.unwrap();
    let reader = Store::open(seat.temp.path().join("estate.sqlite"))
        .await
        .unwrap();
    assert_eq!(reader.entries("strand_test").await.unwrap().len(), 2);
    let (read, ended) = tokio::join!(
        reader.entries("strand_test"),
        seat.store.end_strand("strand_test")
    );
    if let Err(error) = ended {
        assert_eq!(error, "store unavailable");
        assert_eq!(reader.entries("strand_test").await.unwrap().len(), 2);
        seat.store.end_strand("strand_test").await.unwrap();
    }
    match read {
        Ok(entries) => assert!(entries.is_empty() || entries.len() == 2),
        Err(error) => assert!(
            error == "strand not found"
                || error == "store unavailable"
                || error.contains(" missing")
        ),
    }
    assert_eq!(
        reader.entries("strand_test").await.unwrap_err(),
        "strand not found"
    );
}
