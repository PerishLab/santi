#[allow(dead_code)]
#[path = "../../src/client/tui/parse.rs"]
mod parse;

#[allow(dead_code)]
#[path = "../../src/client/tui/parse.rs"]
mod inner;

use inner::read;
use std::future;
use std::time::Duration;

#[tokio::test]
async fn unavailable() {
    let value = read(
        future::pending::<anyhow::Result<serde_json::Value>>(),
        Duration::from_millis(10),
    )
    .await;

    assert_eq!(value, "ctx --");
}
