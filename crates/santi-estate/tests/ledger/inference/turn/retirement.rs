use super::{LATER, choice, drain, start};
use santi_model::catalog::{Definition, Protocol};

#[tokio::test]
async fn retirement() {
    for completed in [true, false] {
        let (_temp, _path, store) = start().await;
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
                protocol: Protocol::ChatCompletions,
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
        let original = choice("legacy", "wire-model", None);
        let turn = drain(&store, "recorded", &original).await.turn;
        assert!(
            store
                .withdraw(alias.clone())
                .await
                .unwrap_err()
                .contains("Inference/")
        );
        if completed {
            store
                .complete_turn(&turn.id, turn.from, LATER)
                .await
                .unwrap();
        } else {
            store.fail_turn(&turn.id, "fixture", LATER).await.unwrap();
        }
        store.withdraw(alias).await.unwrap();
        assert_eq!(store.inference(&turn.id).await.unwrap(), Some(original));
    }
}
