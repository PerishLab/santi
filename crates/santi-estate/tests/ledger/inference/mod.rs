use super::NOW;
use santi_estate::{Store, Strand};
use santi_model::inference::Declaration;

mod catalog;
mod concurrent;
mod fork;
mod store;
mod turn;

fn declaration() -> Declaration {
    Declaration {
        scenario: Some("gateway/deepseek/recall".into()),
        effort: Some("high".into()),
    }
}

async fn parent(store: &Store) -> santi_model::strand::Strand {
    let soul = store.seed("soul", NOW).await.expect("seed");
    let inference = declaration();
    store
        .weave(Strand {
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
