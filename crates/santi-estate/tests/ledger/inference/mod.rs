use super::NOW;
use santi_estate::{Store, StrandDraft};
use santi_model::inference::Declaration;

mod concurrent;
mod fork;
mod store;
mod turn;

fn declaration() -> Declaration {
    Declaration {
        provider: Some("deepseek".into()),
        model: Some("deepseek-chat".into()),
        reasoning_effort: Some("high".into()),
    }
}

async fn parent(store: &Store) -> santi_model::strand::Strand {
    let soul = store.seed("soul", NOW).await.expect("seed");
    let inference = declaration();
    store
        .create_strand(StrandDraft {
            tag: "ss_parent",
            soul: &soul.id,
            label: None,
            parent: None,
            fork: None,
            inference: Some(&inference),
            created: NOW,
        })
        .await
        .expect("parent")
}
