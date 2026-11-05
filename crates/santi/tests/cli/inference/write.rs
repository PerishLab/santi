use serde_json::json;

use super::support::{self, READ};

#[tokio::test]
async fn patches() {
    let home = support::home();

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
            "--provider",
            "edge",
            "--model",
            "m1",
            "--reasoning-effort",
            "high",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "PATCH /api/v1/souls/soul_one/inference HTTP/1.1"
    );
    assert_eq!(
        support::json(&request),
        json!({"provider": "edge", "model": "m1", "reasoning_effort": "high"})
    );

    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["inference", "strand", "set", "ss_one", "--model", "m2"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "PATCH /api/v1/strands/ss_one/inference HTTP/1.1"
    );
    assert_eq!(support::json(&request), json!({"model": "m2"}));
}

#[tokio::test]
async fn clears() {
    let home = support::home();
    let wire = support::serve("200 OK", READ).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &[
            "inference",
            "strand",
            "set",
            "ss_one",
            "--clear",
            "provider,model",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"provider": null, "model": null})
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
            "reasoning-effort",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(support::json(&request), json!({"reasoning_effort": null}));
}
