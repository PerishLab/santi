use super::*;
use serde_json::json;

#[tokio::test]
async fn dispatch() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.unwrap();
    store.seed("soul_test", FIRST).await.unwrap();
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .unwrap();
    let content = message::Content::text("accepted Feishu input");
    store
        .accept_inbox(
            InboxDraft {
                tag: "inbox_test",
                strand: "strand_test",
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .unwrap();
    let intent = json!({"profile":"bot","message":"om_test","emoji":"Get"});
    assert!(store.acknowledge("missing", &intent, FIRST).await.is_err());
    let (a, b) = tokio::join!(
        store.acknowledge("inbox_test", &intent, FIRST),
        store.acknowledge("inbox_test", &intent, FIRST)
    );
    assert_ne!(a.unwrap(), b.unwrap());
    assert!(
        store
            .acknowledge("inbox_test", &json!({"message":"om_other"}), LATER)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .acknowledgement("inbox_test")
            .await
            .unwrap()
            .unwrap()
            .state,
        "dispatching"
    );
    drop(store);
    let store = Store::open(&path).await.unwrap();
    store.interrupted(LATER).await.unwrap();
    assert_eq!(
        store
            .acknowledgement("inbox_test")
            .await
            .unwrap()
            .unwrap()
            .state,
        "unknown"
    );
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert!(
        !store
            .acknowledge("inbox_test", &intent, LATER)
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .acknowledgement("inbox_test")
            .await
            .unwrap()
            .unwrap()
            .intent,
        intent
    );

    assert!(
        store
            .acknowledged("inbox_test", &json!({"state":"accepted"}), LATER)
            .await
            .is_err()
    );
    assert!(
        !store
            .acknowledge("inbox_test", &intent, LATER)
            .await
            .unwrap()
    );
    let observed = store.acknowledgement("inbox_test").await.unwrap().unwrap();
    assert_eq!(observed.state, "unknown");
    assert!(observed.result.is_none());
    let Opening::Started(_) = store
        .drain_turn(DrainDraft {
            turn: "turn_fixture",
            strand: "strand_test",
            trigger: turn::Trigger::StrandSend,
            actor: "santi",
            created: LATER,
        })
        .await
        .unwrap()
    else {
        panic!("accepted input must drain");
    };
    store
        .finish_turn(santi_estate::CompletionDraft {
            turn: "turn_fixture",
            reply: None,
            provider: "test",
            model: "test",
            response: None,
            occurred: LATER,
        })
        .await
        .unwrap();
    assert!(
        store
            .end_strand("strand_test")
            .await
            .unwrap_err()
            .contains("unsettled acknowledgement")
    );

    assert!(
        store
            .confirmed(
                "inbox_test",
                &santi_model::receipt::Confirmation {
                    applied: true,
                    evidence: String::new()
                },
                LATER
            )
            .await
            .is_err()
    );
    store
        .confirmed(
            "inbox_test",
            &santi_model::receipt::Confirmation {
                applied: true,
                evidence: "exact fixture reaction ID/actor/emoji observed".into(),
            },
            LATER,
        )
        .await
        .unwrap();
    let settled = store.acknowledgement("inbox_test").await.unwrap().unwrap();
    assert_eq!(settled.state, "observed");
    assert!(settled.resolution.is_some());
    assert!(
        !store
            .acknowledge("inbox_test", &intent, LATER)
            .await
            .unwrap()
    );
    assert_eq!(
        store.receipt("inbox_test").await.unwrap().unwrap().state,
        santi_model::receipt::State::Completed
    );
    store.end_strand("strand_test").await.unwrap();
    assert!(store.acknowledgement("inbox_test").await.unwrap().is_none());
}

#[tokio::test]
async fn accepted() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .unwrap();
    store.seed("soul_test", FIRST).await.unwrap();
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .unwrap();
    let content = message::Content::text("accepted Feishu input");
    store
        .accept_inbox(
            InboxDraft {
                tag: "inbox_test",
                strand: "strand_test",
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .unwrap();
    let intent = json!({"profile":"bot","message":"om_test","emoji":"Get"});
    assert!(
        store
            .acknowledge("inbox_test", &intent, FIRST)
            .await
            .unwrap()
    );
    let result = json!({"state":"accepted","stdout":"complete SDK result","stderr":"complete diagnostics","exit":0});
    store
        .acknowledged("inbox_test", &result, LATER)
        .await
        .unwrap();
    let observed = store.acknowledgement("inbox_test").await.unwrap().unwrap();
    assert_eq!(observed.state, "accepted");
    assert_eq!(observed.result, Some(result));
    assert!(
        !store
            .acknowledge("inbox_test", &intent, LATER)
            .await
            .unwrap()
    );
    assert!(
        store
            .acknowledged("inbox_test", &json!({"state":"unknown"}), LATER)
            .await
            .is_err()
    );
}
