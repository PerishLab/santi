use super::*;

#[tokio::test]
async fn partial() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer.exchange().await;
    store
        .create_compact(CompactDraft {
            tag: "compact_tail",
            strand: "strand_test",
            first: "m2",
            last: "m3",
            summary: "tail",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect("occupy tail");
    let error = writer.refuse(&named("m1", "m2"), "[compact.overlap]").await;
    assert!(!error.contains("Ready to submit"), "{error}");
    assert!(!error.contains("first: m1, last: m3"), "{error}");
    assert!(!error.contains("first: m1, last: m1"), "{error}");
}
