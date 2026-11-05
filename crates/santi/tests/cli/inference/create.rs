use serde_json::{Value, json};

use super::support;

const CREATED: &str = r#"{"strand":{"id":"ss_new","soul":"soul_one"}}"#;
const FORKED: &str = r#"{"strand":{"id":"ss_child","soul":"soul_one"}}"#;

#[tokio::test]
async fn bodyless() {
    let home = support::home();
    let wire = support::serve("200 OK", CREATED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["strand", "create", "--soul", "soul_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "POST /api/v1/souls/soul_one/strands HTTP/1.1"
    );
    assert_eq!(support::body(&request), "");
}

#[tokio::test]
async fn declared() {
    let home = support::home();
    let wire = support::serve("200 OK", CREATED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &[
            "strand",
            "create",
            "--soul",
            "soul_one",
            "--model",
            "m1",
            "--reasoning-effort",
            "high",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"inference": {"model": "m1", "reasoning_effort": "high"}})
    );

    let wire = support::serve(
        "200 OK",
        r#"{"strand":{"id":"ss_new","soul":"soul_default"}}"#,
    )
    .await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[("SANTI_INFERENCE_PROVIDER", "env-provider")],
        &["strand", "create"],
    )
    .await;
    support::applied(&output);
    assert_eq!(support::line(&request), "POST /api/v1/strands HTTP/1.1");
    assert_eq!(
        support::json(&request),
        json!({"inference": {"provider": "env-provider"}})
    );
}

#[tokio::test]
async fn inherit() {
    let home = support::home();
    let wire = support::serve("200 OK", CREATED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["strand", "create", "--soul", "soul_one", "--inherit"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        Value::Object(
            [("inference".to_string(), Value::Object(Default::default()))]
                .into_iter()
                .collect()
        )
    );
}

#[tokio::test]
async fn copied() {
    let home = support::home();
    let wire = support::serve("200 OK", FORKED).await;
    let (output, request) =
        support::call(wire, home.path(), &[], &["strand", "fork", "ss_parent"]).await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "POST /api/v1/strands/ss_parent/fork HTTP/1.1"
    );
    assert_eq!(support::body(&request), "");

    let wire = support::serve("200 OK", FORKED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[
            ("SANTI_STRAND_ID", "ss_env"),
            ("SANTI_INFERENCE_MODEL", "env-model"),
        ],
        &["strand", "fork"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "POST /api/v1/strands/ss_env/fork HTTP/1.1"
    );
    assert_eq!(
        support::json(&request),
        json!({"inference": {"model": "env-model"}})
    );
}

#[tokio::test]
async fn forked() {
    let home = support::home();
    let wire = support::serve("200 OK", FORKED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &[
            "strand",
            "fork",
            "ss_parent",
            "--provider",
            "edge",
            "--model",
            "m1",
        ],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::json(&request),
        json!({"inference": {"provider": "edge", "model": "m1"}})
    );

    let wire = support::serve("200 OK", FORKED).await;
    let (output, request) = support::call(
        wire,
        home.path(),
        &[],
        &["strand", "fork", "ss_parent", "--inherit"],
    )
    .await;
    support::applied(&output);
    assert_eq!(support::json(&request), json!({"inference": {}}));
}
