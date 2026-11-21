use super::*;
#[tokio::test]
async fn reopen() {
    let (_temp, path, store) = start().await;
    use santi_model::catalog::{Definition as D, Protocol};
    use santi_model::inference::{Label, Route};
    let provider = store
        .register(D::Provider {
            name: "gateway".into(),
        })
        .await
        .unwrap();
    let family = store
        .register(D::Family {
            name: "family".into(),
        })
        .await
        .unwrap();
    let scene = store
        .register(D::Scenario {
            name: "scene".into(),
            criterion: "review".into(),
        })
        .await
        .unwrap();
    let access = store
        .register(D::Access {
            name: "original".into(),
            provider: provider.id,
            protocol: Protocol::ChatCompletions,
            endpoint: "https://fixture.invalid/v1".into(),
            credential: "custody://local".into(),
        })
        .await
        .unwrap();
    let offer = store
        .register(D::Offering {
            access: access.id,
            family: family.id,
            model: "model_a".into(),
        })
        .await
        .unwrap();
    let candidate = store
        .register(D::Candidate {
            scenario: scene.id,
            offering: offer.id,
            evidence: "fixture".into(),
        })
        .await
        .unwrap();
    let mut old = choice("gateway/family/scene", "model_a", Some("low"));
    let named = |id, name: &str| Label {
        id,
        name: name.into(),
    };
    old.source.model = Origin::Offering;
    old.source.route = Some(Box::new(Route {
        provider: named(provider.id, "gateway"),
        access: named(access.id, "original"),
        family: Some(named(family.id, "family")),
        scenario: Some(named(scene.id, "scene")),
        offering: Some(offer.id),
        candidate: Some(candidate.id),
        protocol: "chat_completions".into(),
        endpoint: "https://fixture.invalid/v1".into(),
        custody: "local".into(),
        identity: "fixture".into(),
        policy: "test".into(),
        replay: true,
        thinking: Some("enabled".into()),
    }));
    assert!(store.begin(draft("stale"), &old).await.is_err());
    assert!(store.turn("stale").await.unwrap().is_none());
    assert_eq!(store.inboxes(STRAND).await.unwrap().len(), 1);
    let preference = store
        .register(D::Preference {
            candidate: candidate.id,
        })
        .await
        .unwrap();
    let mut direct = old.clone();
    direct.provider = "original".into();
    let route = direct.source.route.as_mut().unwrap();
    route.scenario = None;
    route.candidate = None;
    assert!(store.begin(draft("direct"), &direct).await.is_err());
    assert!(store.turn("direct").await.unwrap().is_none());
    let one = drain(&store, "turn_one", &old).await;
    assert_eq!(one.turn.inference.as_ref(), Some(&old));
    store
        .complete_turn("turn_one", one.turn.from, LATER)
        .await
        .expect("complete");

    store.withdraw(preference).await.unwrap();
    let blocked = store.withdraw(candidate).await.unwrap_err();
    assert!(blocked.to_string().contains("Route/"));
    let mut renamed = access.definition.clone();
    let D::Access { name, .. } = &mut renamed else {
        unreachable!()
    };
    *name = "renamed".into();
    store.amend(access, renamed).await.unwrap();

    let patch = Patch {
        scenario: Some(Some("gateway/family/next".into())),
        reasoning_effort: Some(Some("high".to_string())),
    };
    store
        .declare(Target::Strand, STRAND, &patch, LATER)
        .await
        .expect("declare");
    assert_eq!(
        store
            .turn("turn_one")
            .await
            .expect("turn")
            .expect("present")
            .inference
            .as_ref(),
        Some(&old)
    );

    accept(&store, "inbox_two", "next").await;
    let next = choice("provider_b", "model_b", Some("high"));
    let two = drain(&store, "turn_two", &next).await;
    assert_eq!(two.turn.inference.as_ref(), Some(&next));
    store
        .complete_turn("turn_two", two.turn.from, LATER)
        .await
        .expect("complete second");

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    for (tag, expected) in [("turn_one", Some(old)), ("turn_two", Some(next))] {
        let turn = store.turn(tag).await.expect("turn").expect("present");
        assert_eq!(turn.inference, expected, "restart snapshot {tag}");
        assert_eq!(
            store.inference(tag).await.expect("inference"),
            expected,
            "restart inference {tag}"
        );
    }
}
