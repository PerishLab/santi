use super::support;

#[tokio::test]
async fn help() {
    let home = support::home();
    let base = "http://127.0.0.1:1";

    let output = support::invoke(base, home.path(), &[], &["inference", "--help"]).await;
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("persistently declare"));
    assert!(text.contains("soul"));
    assert!(text.contains("strand"));

    let output = support::invoke(
        base,
        home.path(),
        &[],
        &["inference", "soul", "set", "--help"],
    )
    .await;
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--clear"));
    assert!(text.contains("inherits again"));

    let output = support::invoke(base, home.path(), &[], &["strand", "fork", "--help"]).await;
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("--inherit"));
    assert!(text.contains("parent's own delta"));
}
