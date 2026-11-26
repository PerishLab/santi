use santi_error::{Draft, Scope, Source};
use santi_estate::{Arrival, Effect, Store, Strand, Turn};
use santi_model::{message, receipt, turn};

pub(super) const FIRST: &str = "2026-07-28T00:00:00.000Z";

pub(super) async fn bootstrap(path: impl AsRef<std::path::Path>) -> Store {
    Store::bootstrap(
        path,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap")
}

pub(super) async fn strand(store: &Store, tag: &str, label: Option<&str>) {
    store
        .weave(Strand {
            tag,
            soul: "soul_test",
            label,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
}

pub(super) async fn driven(store: &Store, suffix: &str) {
    let strand = format!("strand_{suffix}");
    let inbox = format!("inbox_{suffix}");
    let turn = format!("turn_{suffix}");
    let effect = format!("effect_{suffix}");
    let content = message::Content::text("drive");
    store
        .accept(
            Arrival {
                tag: &inbox,
                strand: &strand,
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .expect("inbox");
    self::turn(store, &turn, &strand).await;
    store
        .advance(santi_estate::Receipt {
            inbox: &inbox,
            state: receipt::State::Driving,
            turn: Some(&turn),
            incident: None,
            occurred: FIRST,
        })
        .await
        .expect("receipt");
    store
        .stage(Effect {
            tag: &effect,
            turn: &turn,
            call: None,
            kind: "test",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("effect");
}

pub(super) async fn turn(store: &Store, tag: &str, strand: &str) {
    store
        .start(Turn {
            tag,
            strand,
            trigger: turn::Trigger::System,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
}

pub(super) fn incident(descriptor: santi_error::Descriptor, scope: &Scope, message: &str) -> Draft {
    Draft {
        key: descriptor.key(&scope.kind, &scope.id),
        descriptor,
        scope: scope.clone(),
        source: Source::new("santi-core", "test"),
        message: message.to_string(),
        context: serde_json::json!({"message": message}),
    }
}
