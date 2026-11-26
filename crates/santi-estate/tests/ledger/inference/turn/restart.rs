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
            protocol: Protocol::Chat,
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
    let mut prior = old.clone();
    prior.selection = Selection::Provider("gateway/family/scene".into());
    prior.source.selection = Selection::Provider(Origin::Strand);
    let error = store.begin(draft("prior"), &prior).await.unwrap_err();
    assert!(error.contains("new turn requires a scenario"), "{error}");
    assert!(store.turn("prior").await.unwrap().is_none());
    let mut direct = old.clone();
    let route = direct.source.route.as_mut().unwrap();
    route.scenario = None;
    route.candidate = None;
    assert!(store.begin(draft("direct"), &direct).await.is_err());
    assert!(store.turn("direct").await.unwrap().is_none());
    let legacy = store
        .start(Turn {
            tag: "legacy",
            strand: STRAND,
            trigger: turn::Trigger::System,
            from: 0,
            created: LATER,
        })
        .await
        .unwrap();
    let core = keel::bind(
        santi_estate::graph(),
        keel::adapt::db::Sqlite::file(&path).await.unwrap(),
    )
    .await
    .unwrap();
    let key = core
        .one(&keel::form("Turn").when("tag", keel::Op::Eq, "legacy"))
        .await
        .unwrap()
        .unwrap()
        .key()
        .to_string();
    core.put(
        "Inference",
        &[
            ("turn", &key),
            ("provider", "gateway/family/scene"),
            ("model", "model_a"),
            ("reasoning_effort", "low"),
            (
                "source",
                r#"{"provider":"strand","model":"offering","reasoning_effort":"strand"}"#,
            ),
            ("created", LATER),
        ],
    )
    .await
    .unwrap();
    core.put(
        "Route",
        &[
            ("turn", &key),
            ("provider", &provider.id.to_string()),
            ("provider_name", "gateway"),
            ("access", &access.id.to_string()),
            ("access_name", "original"),
            ("family", &family.id.to_string()),
            ("family_name", "family"),
            ("scenario", &scene.id.to_string()),
            ("scenario_name", "scene"),
            ("offering", &offer.id.to_string()),
            ("candidate", &candidate.id.to_string()),
            ("protocol", "chat_completions"),
            ("endpoint", "https://fixture.invalid/v1"),
            ("custody", "local"),
            ("identity", "fixture"),
            ("policy", "test"),
            ("replay", "true"),
            ("thinking", "enabled"),
        ],
    )
    .await
    .unwrap();
    drop(core);
    store.complete("legacy", legacy.from, LATER).await.unwrap();
    let one = drain(&store, "turn_one", &old).await;
    assert_eq!(one.turn.inference.as_ref(), Some(&old));
    store
        .complete("turn_one", one.turn.from, LATER)
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
        effort: Some(Some("high".to_string())),
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
    let next = choice("fixture/family/b", "model_b", Some("high"));
    let two = drain(&store, "turn_two", &next).await;
    assert_eq!(two.turn.inference.as_ref(), Some(&next));
    store
        .complete("turn_two", two.turn.from, LATER)
        .await
        .expect("complete second");

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    let mut historical = old.clone();
    historical.selection = Selection::Provider("gateway/family/scene".into());
    historical.source.selection = Selection::Provider(Origin::Strand);
    for (tag, expected) in [
        ("legacy", historical),
        ("turn_one", old),
        ("turn_two", next),
    ] {
        let turn = store.turn(tag).await.expect("turn").expect("present");
        let reading = store
            .inference(tag)
            .await
            .expect("inference")
            .expect("present");
        assert_eq!(
            turn.inference.as_ref(),
            Some(&expected),
            "restart snapshot {tag}"
        );
        assert_eq!(reading, expected, "restart inference {tag}");
        let wire = serde_json::to_value(reading).unwrap();
        let (field, absent) = if tag == "legacy" {
            ("provider", "scenario")
        } else {
            ("scenario", "provider")
        };
        assert_eq!(wire[field], expected.selection.value().as_str());
        assert_eq!(wire["source"][field], "strand");
        assert!(wire.get(absent).is_none() && wire["source"].get(absent).is_none());
        if tag != "turn_two" {
            assert_eq!(wire["source"]["route"]["provider"]["name"], "gateway");
            assert_eq!(wire["source"]["route"]["access"]["name"], "original");
        }
    }
}
