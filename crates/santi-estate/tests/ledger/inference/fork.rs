use super::super::{NOW, SUDO};
use super::{declaration, parent};
use santi_estate::{Fork, Store, Strand, Target};
use santi_model::inference::{Declaration, Patch};

#[tokio::test]
async fn fork() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    let parent = parent(&store).await;
    let child = store
        .fork(Fork {
            tag: "ss_child",
            parent: &parent.id,
            at: 0,
            inference: None,
            created: NOW,
        })
        .await
        .expect("fork");
    assert_eq!(child.inference, declaration());
    assert_eq!(child.soul, parent.soul);
    assert_eq!(child.parent.as_deref(), Some(parent.id.as_str()));
    let edited = store
        .declare(
            Target::Strand,
            &parent.id,
            &Patch {
                scenario: Some(Some("gateway/gpt/review".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("edit parent");
    assert_eq!(edited.scenario.as_deref(), Some("gateway/gpt/review"));
    let child = store.strand(&child.id).await.expect("child").expect("held");
    assert_eq!(
        child.inference.scenario.as_deref(),
        Some("gateway/deepseek/recall")
    );
    let explicit = Declaration {
        scenario: Some("gateway/gpt/review".into()),
        effort: None,
    };
    let other = store
        .fork(Fork {
            tag: "ss_other",
            parent: &parent.id,
            at: 0,
            inference: Some(&explicit),
            created: NOW,
        })
        .await
        .expect("fork explicit");
    assert_eq!(other.inference, explicit);
    assert_eq!(other.soul, parent.soul);
    assert!(other.inference.effort.is_none());
    let bare = store
        .weave(Strand {
            tag: "ss_bare",
            soul: &parent.soul,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: NOW,
        })
        .await
        .expect("bare parent");
    let inherited = store
        .fork(Fork {
            tag: "ss_bare_child",
            parent: &bare.id,
            at: 0,
            inference: None,
            created: NOW,
        })
        .await
        .expect("fork bare");
    assert!(inherited.inference.empty());
    assert_eq!(inherited.soul, parent.soul);
}
