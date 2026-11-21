use serde_json::json;

use super::support::{self, READ};

#[tokio::test]
async fn layers() {
    let home = support::home();
    support::file(
        home.path(),
        "[inference]\nscenario = \"file/family/work\"\n",
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"scenario": "file/family/work"})
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[("SANTI_INFERENCE_SCENARIO", "env/family/work")],
        &["inference", "soul", "set", "soul_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"scenario": "env/family/work"})
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[("SANTI_INFERENCE_SCENARIO", "env/family/work")],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--scenario",
            "cli/family/work",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"scenario": "cli/family/work"})
    );
}

#[tokio::test]
async fn environment() {
    let home = support::home();
    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[
            ("SANTI_INFERENCE_SCENARIO", "env/family/work"),
            ("SANTI_INFERENCE_REASONING_EFFORT", "env-effort"),
        ],
        &["inference", "strand", "set", "ss_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({
            "scenario": "env/family/work",
            "reasoning_effort": "env-effort"
        })
    );
}

#[tokio::test]
async fn overrides() {
    let home = support::home();
    support::file(
        home.path(),
        "[inference]\nscenario = \"file/family/work\"\n",
    );
    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--clear",
            "scenario",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(support::json(&request), json!({"scenario": null}));
}
