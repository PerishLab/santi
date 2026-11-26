use super::{LATER, STRAND, choice, start};
use keel::adapt::db::Sqlite;
use santi_estate::Turn;
use santi_model::catalog::{Definition, Protocol};
use santi_model::{
    inference::{Origin, Selection},
    turn::Trigger,
};

#[tokio::test]
async fn retirement() {
    for completed in [true, false] {
        let (_temp, path, store) = start().await;
        let supplier = store
            .register(Definition::Provider {
                name: "supplier".into(),
            })
            .await
            .unwrap();
        let access = store
            .register(Definition::Access {
                name: "chat".into(),
                provider: supplier.id,
                protocol: Protocol::Chat,
                endpoint: "https://example.invalid/v1".into(),
                credential: "custody://chat".into(),
            })
            .await
            .unwrap();
        let alias = store
            .register(Definition::Alias {
                name: "legacy".into(),
                access: access.id,
            })
            .await
            .unwrap();
        let mut original = choice("legacy", "wire-model", None);
        original.selection = Selection::Provider("legacy".into());
        original.source.selection = Selection::Provider(Origin::Strand);
        let turn = store
            .start(Turn {
                tag: "recorded",
                strand: STRAND,
                trigger: Trigger::System,
                from: 0,
                created: LATER,
            })
            .await
            .unwrap();
        let core = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
            .await
            .unwrap();
        let key = core
            .live("Turn")
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.text("tag") == Some("recorded"))
            .unwrap()
            .key()
            .to_string();
        core.put(
            "Inference",
            &[
                ("turn", &key),
                ("provider", "legacy"),
                ("model", "wire-model"),
                ("source", r#"{"provider":"strand","model":"strand"}"#),
                ("created", LATER),
            ],
        )
        .await
        .unwrap();
        drop(core);
        assert!(
            store
                .withdraw(alias.clone())
                .await
                .unwrap_err()
                .contains("Inference/")
        );
        if completed {
            store.complete(&turn.id, turn.from, LATER).await.unwrap();
        } else {
            store.fail(&turn.id, "fixture", LATER).await.unwrap();
        }
        store.withdraw(alias).await.unwrap();
        drop(store);
        let store = santi_estate::Store::open(path).await.unwrap();
        let actual = store.inference(&turn.id).await.unwrap().unwrap();
        assert_eq!(actual, original);
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::json!({
                "provider":"legacy", "model":"wire-model", "source":{"provider":"strand", "model":"strand"}
            })
        );
    }
}
