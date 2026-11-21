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
            "--scenario",
            "edge/family/work",
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
        json!({"scenario": "edge/family/work", "reasoning_effort": "high"})
    );

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
            "--scenario",
            "edge/family/review",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "PATCH /api/v1/strands/ss_one/inference HTTP/1.1"
    );
    assert_eq!(
        support::json(&request),
        json!({"scenario": "edge/family/review"})
    );
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
            "scenario",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(support::json(&request), json!({"scenario": null}));

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
