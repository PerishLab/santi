use super::{FIRST, Fork, LATER, Message, Store, Strand, Turn, message, turn};
use santi_estate::{Call, Reply, Thinking};
use serde_json::json;

#[tokio::test]
async fn boundaries() {
    let temp = tempfile::tempdir().expect("temp");
    let history = History(ready(&temp).await);
    let store = &history.0;
    history.input("input_one").await;
    history.opened("turn_one", 1).await;
    history.called("call_one", "turn_one").await;
    history.called("call_two", "turn_one").await;
    history.replied("reply_one", "call_one").await;
    history.refused(3, "1", "none").await;
    history.refused(8, "1", "none").await;
    history.replied("reply_two", "call_two").await;
    store
        .complete("turn_one", 6, LATER)
        .await
        .expect("complete");
    history.input("input_two").await;
    history.opened("turn_two", 7).await;
    history.called("call_three", "turn_two").await;
    for at in [2, 3, 4, 5] {
        history.refused(at, "1", "6").await;
    }
    for at in [8, 9, 10, i64::MAX] {
        history.refused(at, "7", "none").await;
    }
    history.refused(-1, "none", "0").await;
    assert_eq!(store.strands().await.expect("strands").len(), 1);
    for at in [0, 1, 6, 7] {
        let tag = format!("child_{at}");
        let (child, _) = store
            .positioning(draft(&tag, at), |_, _, _, _| Ok(()))
            .await
            .expect("exact");
        assert_eq!(child.fork, Some(at));
        assert_eq!(child.next, at + 1);
    }
    let (head, _) = store
        .branching(draft("head", 9), |_, _, _, _| Ok(()))
        .await
        .expect("head");
    assert_eq!(
        head.fork,
        Some(6),
        "default checkpoint keeps its existing policy"
    );
}

fn draft(tag: &str, at: i64) -> Fork<'_> {
    Fork {
        tag,
        parent: "parent",
        at,
        inference: None,
        created: LATER,
    }
}

async fn ready(temp: &tempfile::TempDir) -> Store {
    let store = super::super::support::bootstrap(&temp.path().join("estate.sqlite")).await;
    store.seed("soul", FIRST).await.expect("seed");
    store
        .weave(Strand {
            tag: "parent",
            soul: "soul",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    store
}

struct History(Store);

impl History {
    async fn refused(&self, at: i64, earlier: &str, later: &str) {
        let error = self
            .0
            .positioning(draft("refused", at), |_, _, _, _| Ok(()))
            .await
            .expect_err("refused");
        assert!(
            error.contains(&format!("nearest earlier valid position is {earlier}")),
            "{error}"
        );
        assert!(
            error.contains(&format!("nearest later valid position is {later}")),
            "{error}"
        );
        assert!(self.0.strand("refused").await.expect("child").is_none());
    }

    async fn input(&self, tag: &str) {
        self.0
            .place(Message {
                tag,
                strand: "parent",
                actor: message::Role::System,
                sender: "santi",
                kind: message::Kind::Text,
                content: &message::Content::text("input"),
                state: message::State::Fixed,
                request: false,
                created: FIRST,
            })
            .await
            .expect("message");
    }

    async fn opened(&self, tag: &str, from: i64) {
        self.0
            .start(Turn {
                tag,
                strand: "parent",
                trigger: turn::Trigger::Send,
                from,
                created: FIRST,
            })
            .await
            .expect("turn");
        self.0
            .ponder(Thinking {
                tag: &format!("thinking_{tag}"),
                turn: tag,
                response: None,
                created: FIRST,
            })
            .await
            .expect("thinking");
    }

    async fn called(&self, tag: &str, turn: &str) {
        self.0
            .calling(Call {
                tag,
                turn,
                tool: "shell",
                arguments: &json!({"command": "true"}),
                created: FIRST,
            })
            .await
            .expect("call");
    }

    async fn replied(&self, tag: &str, call: &str) {
        self.0
            .replying(Reply {
                tag,
                call,
                output: Some(&json!({"exit":0})),
                error: None,
                created: FIRST,
            })
            .await
            .expect("reply");
    }
}
