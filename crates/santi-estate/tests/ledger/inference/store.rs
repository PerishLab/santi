use super::super::{NOW, SUDO};
use super::{declaration, parent};
use santi_estate::{Store, Strand, Target};
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
                scenario: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("clear scenario");
    assert_eq!(cleared.scenario, None);
    assert_eq!(cleared.effort.as_deref(), Some("high"));
    let empty = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                scenario: Some(None),
                effort: Some(None),
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
        .weave(Strand {
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
                scenario: Some(Some("gateway/deepseek/analysis".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("one field");
    assert_eq!(
        patched.scenario.as_deref(),
        Some("gateway/deepseek/analysis")
    );
    assert_eq!(patched.effort.as_deref(), Some("high"));
    let adjusted = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                scenario: Some(Some("gateway/gpt/recall".into())),
                effort: Some(None),
            },
            NOW,
        )
        .await
        .expect("modify and clear");
    assert_eq!(adjusted.scenario.as_deref(), Some("gateway/gpt/recall"));
    assert_eq!(adjusted.effort, None);
    let error = store
        .declare(
            Target::Strand,
            &strand.id,
            &Patch {
                scenario: Some(Some("  ".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect_err("invalid scenario");
    assert!(error.contains("scenario"), "{error}");
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
                scenario: Some(Some("gateway/gpt/review".into())),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("soul patch");
    assert_eq!(soul.scenario.as_deref(), Some("gateway/gpt/review"));
    assert_eq!(soul.effort, None);
    let soul = store
        .declare(
            Target::Soul,
            "soul",
            &Patch {
                scenario: Some(None),
                ..Default::default()
            },
            NOW,
        )
        .await
        .expect("clear soul scenario");
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

#[tokio::test]
async fn predecessor() {
    use keel::adapt::db::Sqlite;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    drop(Store::bootstrap(&path, SUDO).await.unwrap());
    let core = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
        .await
        .unwrap();
    let id = core
        .put(
            "Soul",
            &[
                ("tag", "consumer"),
                ("created", NOW),
                ("updated", NOW),
                ("provider", "gateway/gpt/review"),
                ("model", "old-model"),
            ],
        )
        .await
        .unwrap();
    let store = Store::open(&path).await.unwrap();
    let error = store.declarations().await.unwrap_err();
    assert!(
        error.contains("consumer") && error.contains("legacy model override"),
        "{error}"
    );
    core.unset("Soul", id, &["model"]).await.unwrap();
    core.set("Soul", id, &[("provider", "direct")])
        .await
        .unwrap();
    assert!(
        store
            .soul("consumer")
            .await
            .unwrap_err()
            .contains("scenario")
    );
    core.set("Soul", id, &[("provider", "gateway/gpt/review")])
        .await
        .unwrap();
    store.declarations().await.unwrap();
    assert_eq!(
        store
            .soul("consumer")
            .await
            .unwrap()
            .unwrap()
            .inference
            .scenario
            .as_deref(),
        Some("gateway/gpt/review")
    );
}
