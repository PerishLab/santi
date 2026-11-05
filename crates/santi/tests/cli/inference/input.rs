use serde_json::json;

use super::support::{self, READ};

#[tokio::test]
async fn layers() {
    let home = support::home();
    support::file(
        home.path(),
        "[inference]\nprovider = \"file-provider\"\nmodel = \"file-model\"\n",
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
        json!({"provider": "file-provider", "model": "file-model"})
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[("SANTI_INFERENCE_MODEL", "env-model")],
        &["inference", "soul", "set", "soul_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"provider": "file-provider", "model": "env-model"})
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[("SANTI_INFERENCE_MODEL", "env-model")],
        &[
            "inference",
            "soul",
            "set",
            "soul_one",
            "--provider",
            "cli-provider",
            "--model",
            "cli-model",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"provider": "cli-provider", "model": "cli-model"})
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
            ("SANTI_INFERENCE_PROVIDER", "env-provider"),
            ("SANTI_INFERENCE_MODEL", "env-model"),
            ("SANTI_INFERENCE_REASONING_EFFORT", "env-effort"),
        ],
        &["inference", "strand", "set", "ss_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({
            "provider": "env-provider",
            "model": "env-model",
            "reasoning_effort": "env-effort"
        })
    );
}

#[tokio::test]
async fn overrides() {
    let home = support::home();
    support::file(
        home.path(),
        "[inference]\nprovider = \"file-provider\"\nmodel = \"file-model\"\n",
    );
    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["inference", "soul", "set", "soul_one", "--clear", "model"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"provider": "file-provider", "model": null})
    );
}
