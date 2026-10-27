use super::*;
use super::{fixture::Frame, value};
use serde_json::json;

#[tokio::test]
async fn inputs() {
    let frame = Frame::new().await;
    assert!(frame.expected.len() > 400);
    let input = value::input(provider_input(&frame.store, "strand_test").await.unwrap());
    assert_eq!(input, frame.expected);
    let end = frame
        .store
        .strand("strand_test")
        .await
        .unwrap()
        .unwrap()
        .next
        - 1;
    frame
        .store
        .fork(ForkDraft {
            tag: "strand_child",
            parent: "strand_test",
            at: end,
            created: LATER,
        })
        .await
        .unwrap();
    assert_eq!(
        value::input(provider_input(&frame.store, "strand_child").await.unwrap()),
        frame.expected
    );
    let reopened = Store::open(frame.temp.path().join("estate.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        value::input(provider_input(&reopened, "strand_test").await.unwrap()),
        frame.expected
    );
    frame
        .store
        .create_compact(CompactDraft {
            tag: "compact_partial",
            strand: "strand_test",
            first: "message_first",
            last: "message_middle",
            summary: "partial",
            metadata: None,
            expected: None,
            created: LATER,
        })
        .await
        .unwrap();
    let partial = value::input(provider_input(&frame.store, "strand_test").await.unwrap());
    assert_eq!(&partial[1..], &frame.expected[frame.middle + 1..]);
    let header = value::header(&partial[0]);
    assert_eq!(header["compact"], "compact_partial");
    assert_eq!(header["covered_strand_seq"]["start_seq"], 1);
    assert_eq!(header["covered_strand_seq"]["end_seq"], frame.middle + 1);
    assert_eq!(
        value::input(provider_input(&frame.store, "strand_child").await.unwrap()),
        frame.expected
    );
    let report = frame
        .store
        .preview_compact(
            "compact_preview",
            "strand_test",
            "message_first",
            "message_last",
        )
        .await
        .unwrap();
    let preview = value::input(
        provider_preview(
            &frame.store,
            "strand_test",
            santi_core::ProviderPreview {
                report: &report,
                summary: "full 工具🙂",
                metadata: &json!({}),
            },
        )
        .await
        .unwrap(),
    );
    assert_eq!(preview.len(), 1);
    let header = value::header(&preview[0]);
    assert_eq!(header["compact"], "compact_preview");
    assert_eq!(header["covered_strand_seq"]["end_seq"], end);
    assert_eq!(header["collapsed_entries"], frame.expected.len());
    frame
        .store
        .create_compact(CompactDraft {
            tag: "compact_full",
            strand: "strand_test",
            first: "message_first",
            last: "message_last",
            summary: "full 工具🙂",
            metadata: None,
            expected: None,
            created: LATER,
        })
        .await
        .unwrap();
    let compacted = value::input(provider_input(&frame.store, "strand_test").await.unwrap());
    assert_eq!(compacted.len(), 1);
    assert_eq!(value::header(&compacted[0])["compact"], "compact_full");
    assert_eq!(
        value::header(&compacted[0])["covered_strand_seq"]["end_seq"],
        end
    );
    assert!(
        compacted[0]["content"]
            .as_str()
            .unwrap()
            .contains("<compact_summary>\nfull 工具🙂")
    );
    assert_eq!(
        value::input(provider_input(&frame.store, "strand_child").await.unwrap()),
        frame.expected
    );
}
