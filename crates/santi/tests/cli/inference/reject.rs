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
        &["inference", "soul", "set", "soul_one", "--scenario", "chat"],
    )
    .await;
    support::refused(&output, "requires provider/family/scene");

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--scenario",
            "edge/family/work",
            "--clear",
            "scenario",
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

    for key in ["SANTI_INFERENCE_PROVIDER", "SANTI_INFERENCE_MODEL"] {
        let output =
            support::invoke(base, home.path(), &[(key, "direct")], &["strand", "create"]).await;
        support::refused(&output, key);
    }

    for field in ["provider", "model"] {
        support::file(home.path(), &format!("[inference]\n{field} = \"wire\"\n"));
        let output = support::invoke(base, home.path(), &[], &["strand", "create"]).await;
        support::refused(&output, field);
    }

    support::file(home.path(), "[inference]\ntempo = \"x\"\n");
    let output = support::invoke(
        base,
        home.path(),
        &[],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--scenario",
            "edge/family/work",
        ],
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
