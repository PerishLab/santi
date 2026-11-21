use super::super::{NOW, SUDO};
use santi_estate::{Store, Target};
use santi_model::catalog::{Definition as D, Protocol, selection};
use santi_model::inference::Patch;

#[tokio::test]
async fn choices() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("estate.sqlite");
    let s = Store::bootstrap(&path, SUDO).await.unwrap();
    let provider = s
        .register(D::Provider {
            name: "teamorouter".into(),
        })
        .await
        .unwrap();
    let family = s
        .register(D::Family {
            name: "gemini".into(),
        })
        .await
        .unwrap();
    let scene = s
        .register(D::Scenario {
            name: "review".into(),
            criterion: "recall a tool result".into(),
        })
        .await
        .unwrap();
    let mut candidates = Vec::new();
    for (name, protocol) in [
        ("chat", Protocol::ChatCompletions),
        ("native", Protocol::Gemini),
    ] {
        let access = s
            .register(D::Access {
                name: name.into(),
                provider: provider.id,
                protocol,
                endpoint: "https://example.invalid/v1".into(),
                credential: "custody://example".into(),
            })
            .await
            .unwrap();
        let alias = s
            .register(D::Alias {
                name: format!("legacy{name}"),
                access: access.id,
            })
            .await
            .unwrap();
        assert!(
            s.withdraw(access.clone())
                .await
                .unwrap_err()
                .contains("Alias")
        );
        s.withdraw(alias).await.unwrap();
        let offering = s
            .register(D::Offering {
                access: access.id,
                family: family.id,
                model: "neon".into(),
            })
            .await
            .unwrap();
        let candidate = s
            .register(D::Candidate {
                scenario: scene.id,
                offering: offering.id,
                evidence: "fixture".into(),
            })
            .await
            .unwrap();
        candidates.push(candidate.id);
    }
    let initial = s.sources().await.unwrap();
    assert_eq!(
        selection::Directory(&initial)
            .explain("teamorouter", "gemini", "review")
            .unwrap()
            .preferred,
        None
    );
    let peer = Store::open(&path).await.unwrap();
    let (left, right) = tokio::join!(
        s.register(D::Preference {
            candidate: candidates[0]
        }),
        peer.register(D::Preference {
            candidate: candidates[1]
        })
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let pref = left.or(right).unwrap();
    let selector = "teamorouter/gemini/review";
    assert!(
        s.withdrawing(pref.clone(), Some(selector))
            .await
            .unwrap_err()
            .contains("service declaration")
    );
    s.seed("consumer", NOW).await.unwrap();
    s.declare(
        Target::Soul,
        "consumer",
        &Patch {
            scenario: Some(Some(selector.into())),
            ..Default::default()
        },
        NOW,
    )
    .await
    .unwrap();
    let renamed = D::Family { name: "new".into() };
    assert!(
        s.amend(family.clone(), renamed.clone())
            .await
            .unwrap_err()
            .contains("Soul/")
    );
    assert!(
        s.withdraw(pref.clone())
            .await
            .unwrap_err()
            .contains("Soul/")
    );
    s.amend(
        pref,
        D::Preference {
            candidate: candidates[1],
        },
    )
    .await
    .unwrap();
    s.declare(
        Target::Soul,
        "consumer",
        &Patch {
            scenario: Some(None),
            ..Default::default()
        },
        NOW,
    )
    .await
    .unwrap();
    s.amend(family.clone(), renamed).await.unwrap();
    assert!(
        s.amend(
            family,
            D::Family {
                name: "raced".into()
            }
        )
        .await
        .is_err()
    );
    drop(s);
    drop(peer);
    let rows = Store::open(path).await.unwrap().sources().await.unwrap();
    let choice = selection::Directory(&rows)
        .explain("teamorouter", "new", "review")
        .unwrap();
    assert_eq!(choice.preferred, Some(candidates[1]));
    assert_eq!(choice.candidates, candidates);
}
