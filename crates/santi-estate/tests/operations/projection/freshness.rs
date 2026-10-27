use super::support::*;
use santi_estate::{CallDraft, ReplyDraft, Store};
use serde_json::json;

#[tokio::test]
async fn writes() {
    let seat = Seat::new().await;
    seat.pair(0).await;
    let reader = Store::open(seat.temp.path().join("estate.sqlite"))
        .await
        .unwrap();
    assert_eq!(reader.calls("strand_test").await.unwrap().len(), 1);
    assert_eq!(reader.results("strand_test").await.unwrap().len(), 1);
    seat.pair(1).await;
    assert_eq!(reader.calls("strand_test").await.unwrap().len(), 2);
    assert_eq!(reader.results("strand_test").await.unwrap().len(), 2);
    for revision in 0..12 {
        let tag = format!("call_new_{revision}");
        let arguments = json!({"revision": revision});
        let draft = CallDraft {
            tag: &tag,
            turn: "turn_test",
            tool: "shell",
            arguments: &arguments,
            created: NOW,
        };
        let (read, write) =
            tokio::join!(reader.calls("strand_test"), seat.store.create_call(draft));
        let call = write.expect("independent committed call");
        match read {
            Ok(calls) => assert!((2 + revision..=3 + revision).contains(&calls.len())),
            Err(error) => assert_eq!(error, "store unavailable"),
        }
        let calls = reader.calls("strand_test").await.unwrap();
        assert_eq!(calls.last().unwrap().id, call.id);
        assert_eq!(calls.last().unwrap().arguments, arguments);
        seat.store
            .create_reply(ReplyDraft {
                tag: &format!("result_new_{revision}"),
                call: &tag,
                output: Some(&arguments),
                error: None,
                created: NOW,
            })
            .await
            .unwrap();
        assert_eq!(
            reader
                .results("strand_test")
                .await
                .unwrap()
                .last()
                .unwrap()
                .output,
            Some(arguments)
        );
    }
}
