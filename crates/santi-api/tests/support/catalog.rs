use santi_core::Store;
use santi_core::sources::{Definition, Protocol};

pub async fn qualify(store: &Store, endpoint: &str, scenarios: &[(&str, &str)]) {
    let provider = store
        .register(Definition::Provider {
            name: "gateway".into(),
        })
        .await
        .unwrap();
    let family = store
        .register(Definition::Family {
            name: "family".into(),
        })
        .await
        .unwrap();
    let access = store
        .register(Definition::Access {
            name: "operator".into(),
            provider: provider.id,
            protocol: Protocol::OpenAiResponses,
            endpoint: endpoint.into(),
            credential: "custody://operator".into(),
        })
        .await
        .unwrap();
    for &(name, model) in scenarios {
        let scenario = store
            .register(Definition::Scenario {
                name: name.into(),
                criterion: format!("Exercise the {name} fixture"),
            })
            .await
            .unwrap();
        let offering = store
            .register(Definition::Offering {
                access: access.id,
                family: family.id,
                model: model.into(),
            })
            .await
            .unwrap();
        let candidate = store
            .register(Definition::Candidate {
                scenario: scenario.id,
                offering: offering.id,
                evidence: "service contract fixture".into(),
            })
            .await
            .unwrap();
        store
            .register(Definition::Preference {
                candidate: candidate.id,
            })
            .await
            .unwrap();
    }
}
