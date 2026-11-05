use super::super::{NOW, SUDO};
use super::{declaration, parent};
use santi_estate::{Store, StrandDraft, Target};
use santi_model::inference::Patch;

#[tokio::test]
async fn roundtrip() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    let strand = parent(&store).await;
    assert_eq!(strand.inference, declaration());
    let soul = store.soul("soul").await.expect("soul").expect("found");
    assert!(soul.inference.empty());
    let cleared = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                model: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("clear model");
    assert_eq!(cleared.provider.as_deref(), Some("deepseek"));
    assert_eq!(cleared.model, None);
    assert_eq!(cleared.reasoning_effort.as_deref(), Some("high"));
    let empty = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                provider: Some(None),
                reasoning_effort: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("clear rest");
    assert!(empty.empty());

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    assert!(
        store
            .strand(&strand.id)
            .await
            .expect("read")
            .expect("held")
            .inference
            .empty()
    );
}

#[tokio::test]
async fn created() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    let soul = store.seed("old_soul", NOW).await.expect("seed");
    let strand = store
        .create_strand(StrandDraft {
            tag: "ss_old",
            soul: &soul.id,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: NOW,
        })
        .await
        .expect("strand");
    assert!(strand.inference.empty());
    assert!(
        store
            .soul(&soul.id)
            .await
            .expect("soul")
            .expect("held")
            .inference
            .empty()
    );
}

#[tokio::test]
async fn atomic() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    let strand = parent(&store).await;
    let patched = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                model: Some(Some("next-model".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("one field");
    assert_eq!(patched.provider.as_deref(), Some("deepseek"));
    assert_eq!(patched.model.as_deref(), Some("next-model"));
    assert_eq!(patched.reasoning_effort.as_deref(), Some("high"));
    let adjusted = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                model: Some(Some("final-model".into())),
                reasoning_effort: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("modify and clear");
    assert_eq!(adjusted.provider.as_deref(), Some("deepseek"));
    assert_eq!(adjusted.model.as_deref(), Some("final-model"));
    assert_eq!(adjusted.reasoning_effort, None);
    let error = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                model: Some(Some("  ".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect_err("invalid model");
    assert!(error.contains("must not be empty"), "{error}");
    let unchanged = store
        .strand(&strand.id)
        .await
        .expect("strand")
        .expect("held");
    assert_eq!(unchanged.inference, adjusted);
    let soul = store
        .declare(
            Target::Soul,
            "soul",
            &Patch {
                provider: Some(Some("openai".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("soul patch");
    assert_eq!(soul.provider.as_deref(), Some("openai"));
    assert_eq!(soul.model, None);
    assert_eq!(soul.reasoning_effort, None);
    let soul = store
        .declare(
            Target::Soul,
            "soul",
            &Patch {
                provider: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("clear soul provider");
    assert!(soul.empty());

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    assert_eq!(
        store
            .strand(&strand.id)
            .await
            .expect("strand")
            .expect("held")
            .inference,
        adjusted
    );
    assert_eq!(
        store
            .soul("soul")
            .await
            .expect("soul")
            .expect("held")
            .inference,
        soul
    );
}
