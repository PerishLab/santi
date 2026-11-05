use serde_json::Value;

use super::support::{self, READ};

struct Case<'a> {
    scope: &'a str,
    path: &'a str,
}

async fn scope(case: Case<'_>, id: Option<&str>, env: &[(&str, &str)]) {
    let Case { scope, path } = case;
    let home = support::home();
    let wire = support::serve("200 OK", READ).await;
    let mut args = vec!["inference", scope, "get"];
    if let Some(id) = id {
        args.push(id);
    }
    let (output, request) = support::call(wire, home.path(), env, &args).await;
    support::applied(&output);
    let id = id.unwrap_or_else(|| env.first().map(|(_, value)| *value).unwrap_or("unused"));
    assert_eq!(
        support::line(&request),
        format!("GET /api/v1/{path}/{id}/inference HTTP/1.1")
    );
    assert_eq!(support::body(&request), "");
    let printed: Value = serde_json::from_slice(&output.stdout).expect("json stdout");
    assert_eq!(printed, serde_json::from_str::<Value>(READ).unwrap());
}

#[tokio::test]
async fn reads() {
    scope(
        Case {
            scope: "soul",
            path: "souls",
        },
        Some("soul_one"),
        &[],
    )
    .await;
    scope(
        Case {
            scope: "strand",
            path: "strands",
        },
        Some("ss_one"),
        &[],
    )
    .await;
    scope(
        Case {
            scope: "soul",
            path: "souls",
        },
        None,
        &[("SANTI_SOUL_ID", "soul_env")],
    )
    .await;
    scope(
        Case {
            scope: "strand",
            path: "strands",
        },
        None,
        &[("SANTI_STRAND_ID", "ss_env")],
    )
    .await;
}

#[tokio::test]
async fn ignores() {
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
        &["inference", "soul", "get", "soul_one"],
    )
    .await;
    support::applied(&output);
    assert_eq!(
        support::line(&request),
        "GET /api/v1/souls/soul_one/inference HTTP/1.1"
    );
    assert_eq!(support::body(&request), "");
}
