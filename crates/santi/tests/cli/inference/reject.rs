use super::support;

#[tokio::test]
async fn rejects() {
    let home = support::home();
    let base = "http://127.0.0.1:1";

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one"],
    )
    .await;
    support::refused(&output, "requires a field value or --clear");

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one", "--provider", " "],
    )
    .await;
    support::refused(&output, "must not be empty");

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--model",
            "m",
            "--clear",
            "model",
        ],
    )
    .await;
    support::refused(&output, "cannot be both set and cleared");

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one", "--clear", "tempo"],
    )
    .await;
    assert_eq!(output.status.code(), Some(2));
    support::refused(&output, "invalid value");

    support::file(home.path(), "[inference]\ntempo = \"x\"\n");
    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one", "--model", "m"],
    )
    .await;
    support::refused(&output, "tempo");

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["strand", "send", "--model", "m", "ss_one", "hello"],
    )
    .await;
    assert_eq!(output.status.code(), Some(2));
    support::refused(&output, "--model");
}
