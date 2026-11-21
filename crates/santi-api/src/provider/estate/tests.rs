use super::*;
use santi_core::provider::Resolver;
use santi_core::sources::Protocol;
use santi_core::thinking::Continuation;
use santi_provider::{Item, Request};

fn catalog() -> Catalog<'static> {
    let profiles = toml::from_str(
        r#"
[operator]
kind = "gemini_generate_content"
api_key = "fixture-only"
model = "profile-default"
base_url = "https://fixture.invalid/v1"
bytes = 1000
"#,
    )
    .unwrap();
    Catalog {
        profiles: Box::leak(Box::default()),
        custody: Box::leak(Box::new(profiles)),
    }
}

fn graph() -> Vec<Record> {
    let access = |name: &str| D::Access {
        name: name.into(),
        provider: 1,
        protocol: Protocol::Gemini,
        endpoint: "https://fixture.invalid/v1".into(),
        credential: "custody://operator".into(),
    };
    vec![
        D::Provider {
            name: "gateway".into(),
        },
        D::Family {
            name: "family".into(),
        },
        D::Scenario {
            name: "review".into(),
            criterion: "test".into(),
        },
        access("direct"),
        D::Offering {
            access: 4,
            family: 2,
            model: "offered".into(),
        },
        D::Candidate {
            scenario: 3,
            offering: 5,
            evidence: "fixture".into(),
        },
        D::Alias {
            name: "legacy".into(),
            access: 4,
        },
        access("alternate"),
        D::Offering {
            access: 8,
            family: 2,
            model: "offered".into(),
        },
        D::Candidate {
            scenario: 3,
            offering: 9,
            evidence: "fixture".into(),
        },
        D::Preference { candidate: 6 },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, definition)| Record {
        id: i as i64 + 1,
        definition,
    })
    .collect()
}

#[test]
fn continuity() {
    let resolver = catalog();
    let mut rows = graph();
    let service = Declaration {
        provider: Some("legacy".into()),
        model: Some("old-default".into()),
        ..Default::default()
    };
    let soul = Declaration {
        provider: Some("gateway/family/review".into()),
        ..Default::default()
    };
    let strand = Declaration {
        reasoning_effort: Some("low".into()),
        ..Default::default()
    };
    let layers = [
        (&service, Origin::Service),
        (&soul, Origin::Soul),
        (&strand, Origin::Strand),
    ];
    let selected = resolver.selecting(&layers, &rows).unwrap();
    assert_eq!(selected.source.provider, Origin::Soul);
    assert_eq!(selected.source.model, Origin::Offering);
    assert_eq!(selected.source.reasoning_effort, Some(Origin::Strand));
    let route = selected.source.route.as_ref().unwrap();
    assert_eq!(route.candidate, Some(6));
    assert_eq!(route.offering, Some(5));
    assert_eq!(route.access.name, "direct");
    let actual = resolver.build(&selected).unwrap().metadata();
    assert_eq!(actual.model, "offered");
    let request = Request {
        strand: None,
        model: actual.model.clone(),
        instructions: None,
        tools: None,
        previous: None,
        input: vec![Item::Continuation(Continuation {
            calls: 0,
            provider: actual.provider.to_string(),
            model: actual.model,
            endpoint: route.endpoint.clone(),
            content: None,
            parts: Some(vec![
                serde_json::json!({"text":"kept", "thoughtSignature":"fixture"}),
            ]),
        })],
    };
    let D::Access { name, .. } = &mut rows[3].definition else {
        unreachable!()
    };
    *name = "renamed".into();
    let renamed = resolver.selecting(&layers, &rows).unwrap();
    resolver
        .build(&renamed)
        .unwrap()
        .validate(&request)
        .unwrap();
    rows.last_mut().unwrap().definition = D::Preference { candidate: 10 };
    let changed = resolver.selecting(&layers, &rows).unwrap();
    assert!(
        resolver
            .build(&changed)
            .unwrap()
            .validate(&request)
            .is_err()
    );
    catalog()
        .build(&selected)
        .unwrap()
        .validate(&request)
        .unwrap();
    assert_eq!(route.access.name, "direct");
    rows.pop();
    assert!(
        resolver
            .selecting(&layers, &rows)
            .unwrap_err()
            .contains("no preferred candidate")
    );
}

#[test]
fn explicit() {
    let resolver = catalog();
    let mut rows = graph();
    let direct = Declaration {
        provider: Some("legacy".into()),
        model: Some("unlisted".into()),
        ..Default::default()
    };
    let resolve = |rows: &[Record]| resolver.selecting(&[(&direct, Origin::Strand)], rows);
    let selected = resolve(&rows).unwrap();
    let route = selected.source.route.as_ref().unwrap();
    assert_eq!(route.access.id, 4);
    assert!(route.offering.is_none() && route.family.is_none() && route.candidate.is_none());
    assert_eq!(
        resolver.build(&selected).unwrap().metadata().model,
        "unlisted"
    );
    let original = rows[3].clone();
    for change in ["endpoint", "protocol", "handle", "unknown"] {
        rows[3] = original.clone();
        let D::Access {
            endpoint,
            protocol,
            credential,
            ..
        } = &mut rows[3].definition
        else {
            unreachable!()
        };
        match change {
            "endpoint" => *endpoint = "https://elsewhere.invalid/v1".into(),
            "protocol" => *protocol = Protocol::ChatCompletions,
            "handle" => *credential = "env://NOT_ALLOWED".into(),
            _ => *credential = "custody://missing".into(),
        }
        assert!(resolve(&rows).is_err(), "{change}");
    }
    let mut changed = selected.clone();
    changed
        .source
        .route
        .as_mut()
        .unwrap()
        .endpoint
        .push_str("/other");
    assert!(resolver.build(&changed).is_err());
}
mod migration;
