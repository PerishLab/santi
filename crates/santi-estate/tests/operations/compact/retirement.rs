use super::FIRST;
use santi_estate::{Compact, Message, Strand};
use santi_model::{compact, message};

#[tokio::test]
async fn reserved() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = super::super::support::bootstrap(&path).await;
    store.seed("soul_test", FIRST).await.expect("seed");
    for (strand, tag) in [
        ("strand_gone", "message_gone"),
        ("strand_kept", "message_kept"),
    ] {
        store
            .weave(Strand {
                tag: strand,
                soul: "soul_test",
                label: None,
                parent: None,
                fork: None,
                inference: None,
                created: FIRST,
            })
            .await
            .expect("strand");
        store
            .place(Message {
                tag,
                strand,
                actor: message::Role::System,
                sender: "santi",
                kind: message::Kind::Text,
                content: &message::Content::text("boundary"),
                state: message::State::Fixed,
                request: false,
                created: FIRST,
            })
            .await
            .expect("message");
    }
    let draft = Compact {
        tag: "compact_gone",
        strand: "strand_gone",
        first: "message_gone",
        last: "message_gone",
        summary: "summary",
        metadata: None,
        expected: None,
        created: FIRST,
    };
    store.condense(draft).await.expect("compact");
    store.retire("strand_gone").await.expect("retire");

    let reused = store
        .condense(Compact {
            tag: "compact_gone",
            strand: "strand_kept",
            first: "message_kept",
            last: "message_kept",
            summary: "replacement",
            metadata: None,
            expected: None,
            created: FIRST,
        })
        .await
        .expect_err("retired ID remains reserved");
    assert!(reused.contains("compact compact_gone has already retired"));
    assert!(matches!(
        store.lookup("compact_gone", None, 0, 10).await.expect("lookup"),
        compact::Lookup::Retired { strand } if strand == "strand_gone"
    ));
}
