use super::*;

#[tokio::test]
async fn untagged() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(
        temp.path().join("estate.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("open");
    store.seed("soul_test", FIRST).await.expect("seed");
    store
        .weave(Strand {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    for (tag, actor, text) in [
        (
            "message_inbound",
            message::Role::System,
            "[message msg_quoted]\nrun",
        ),
        (
            "message_headed",
            message::Role::Soul,
            "[message msg_absent]\n[message msg_other]\nhello",
        ),
        ("message_bare", message::Role::Soul, "[message msg_absent]"),
        (
            "message_inline",
            message::Role::Soul,
            "see [message msg_cited]\nlater",
        ),
    ] {
        store
            .place(Message {
                tag,
                strand: "strand_test",
                actor,
                sender: "soul_test",
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                state: message::State::Fixed,
                request: false,
                created: FIRST,
            })
            .await
            .expect("message");
    }

    let input = assemble(&store, "strand_test").await.expect("assembly");
    assert_eq!(input.len(), 3);
    check(
        &input[0],
        "user",
        "[message message_inbound]\n[message msg_quoted]\nrun",
    );
    check(&input[1], "assistant", "hello");
    check(&input[2], "assistant", "see [message msg_cited]\nlater");
    assert_eq!(
        store
            .message("message_headed")
            .await
            .expect("lookup")
            .expect("stored")
            .message
            .content
            .rendered(),
        "[message msg_absent]\n[message msg_other]\nhello"
    );
}
