use super::*;
use santi_estate::{Capture, ThinkingDraft, TurnDraft};

#[tokio::test]
async fn retained() {
    let fixture = Fixture::open().await;
    let package = package("immutable guide");
    fixture.bind(Scope::Strand, "load", &package).await.unwrap();
    fixture
        .store
        .create_turn(TurnDraft {
            tag: "turn",
            strand: "first",
            trigger: santi_model::turn::Trigger::System,
            from: 1,
            created: NOW,
        })
        .await
        .unwrap();
    let selected = fixture.selection("first").await;
    let component = material::Component {
        reference: package.material.reference.clone(),
        revision: package.material.revision.clone(),
        entry: package.material.entry.clone(),
        origins: vec![material::Origin {
            scope: Scope::Strand,
            provenance: package.material.provenance.clone(),
        }],
        bytes: 15,
    };
    let capture = fixture
        .store
        .capture(Capture {
            tag: "prepared",
            turn: "turn",
            round: 1,
            selection: &selected,
            prompt: "prompt immutable guide",
            rendered: "immutable guide",
            components: &[component],
            created: NOW,
        })
        .await
        .unwrap();
    assert!(capture.thinking.is_none());
    fixture
        .store
        .create_thinking(ThinkingDraft {
            tag: "thinking",
            turn: "turn",
            response: None,
            created: NOW,
        })
        .await
        .unwrap();
    fixture.store.contextualize("thinking").await.unwrap();
    fixture.drop(Scope::Strand, "drop").await;
    assert_eq!(
        fixture.store.archived("first", "thinking").await.unwrap(),
        vec![package]
    );
    let context = fixture
        .store
        .context("first", "prepared")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(context.thinking.as_deref(), Some("thinking"));
    assert_eq!(context.materials.len(), 1);
    assert!(fixture.store.context("second", "prepared").await.is_err());
    assert_eq!(fixture.store.contexts("first").await.unwrap().len(), 1);
}
