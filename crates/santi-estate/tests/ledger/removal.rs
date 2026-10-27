use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use keel::adapt::db::Sqlite;
use santi_error::Scope;
use santi_estate::{EnvironDraft, ForkDraft, ReplayDraft, Store};
use santi_model::{environ, message};

use super::seed::{FIRST, LATER, SOUL, SUDO, failure, inbox, populate, seed};

const SOULED: [&str; 7] = [
    "Soul",
    "Webhook",
    "Downstream",
    "OutboxStream",
    "SoulEnviron",
    "SoulIncident",
    "TraceRecord",
];

#[tokio::test]
async fn owned() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    seed(&store).await;
    store
        .set_environ(EnvironDraft {
            scope: environ::Scope::Soul,
            owner: SOUL,
            name: "SOUL_VALUE",
            value: "kept",
            occurred: FIRST,
        })
        .await
        .expect("soul environ");
    store
        .raise(failure(&Scope::new("soul", SOUL)), FIRST)
        .await
        .expect("soul incident");
    populate(&store, "kept").await;
    populate(&store, "gone").await;
    settled(&store, "gone").await;
    settled(&store, "kept").await;
    let before = live(&path).await;

    store.end_strand("gone").await.expect("end strand");

    assert!(store.strand("gone").await.expect("strand").is_none());
    assert!(store.materials("gone").await.expect("materials").is_empty());
    assert_eq!(store.materials("kept").await.expect("materials").len(), 1);
    let after = live(&path).await;
    dangling(&path).await;
    for (unit, keys) in &before {
        let kept = &after[unit];
        if SOULED.contains(&unit.as_str()) {
            assert_eq!(kept, keys, "{unit} changed");
        } else if !keys.is_empty() {
            assert!(kept.len() < keys.len(), "{unit} kept every row");
            assert!(!kept.is_empty(), "{unit} lost the other strand's rows");
        }
    }
    let hello = message::Content::text("hello");
    assert!(
        store
            .accept_replay(
                inbox("inbox_late", "kept", &hello),
                ReplayDraft::Webhook {
                    subscription: "github",
                    delivery: "delivery_gone",
                    digest: "request_gone",
                },
                10,
            )
            .await
            .expect("redelivery")
            .inserted
    );
}

#[tokio::test]
async fn forked() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    seed(&store).await;
    populate(&store, "parent").await;
    let parent = store.strand("parent").await.expect("parent").expect("live");
    let child = store
        .fork(ForkDraft {
            tag: "child",
            parent: "parent",
            at: parent.next - 1,
            created: LATER,
        })
        .await
        .expect("fork");
    let shown = store.messages(&child.id).await.expect("shown").len();
    assert!(shown > 0);

    let refused = store.end_strand("parent").await.expect_err("refused");
    assert!(refused.contains("child"), "{refused}");
    assert!(store.strand("parent").await.expect("parent").is_some());
    assert_eq!(store.messages("child").await.expect("child").len(), shown);

    store.end_strand("child").await.expect("end child");
    settled(&store, "parent").await;
    store.end_strand("parent").await.expect("end parent");
    dangling(&path).await;
}

async fn live(path: &Path) -> BTreeMap<String, BTreeSet<i64>> {
    let core = keel::bind(
        santi_estate::graph(),
        Sqlite::file(path).await.expect("wire"),
    )
    .await
    .expect("bind")
    .share();
    let mut live = BTreeMap::new();
    for spec in santi_estate::graph().nodes().values() {
        let rows = core.ask(&keel::form(spec.name())).await.expect("rows");
        let keys = rows.rows().iter().map(keel::Row::key).collect();
        live.insert(spec.name().to_string(), keys);
    }
    live
}

async fn dangling(path: &Path) {
    let core = keel::bind(
        santi_estate::graph(),
        Sqlite::file(path).await.expect("wire"),
    )
    .await
    .expect("bind")
    .share();
    let live = live(path).await;
    for spec in santi_estate::graph().nodes().values() {
        let rows = core.ask(&keel::form(spec.name())).await.expect("rows");
        for row in rows.rows() {
            held(spec, row, &live);
        }
    }
}

fn held(spec: &keel::spec::Spec, row: &keel::Row, live: &BTreeMap<String, BTreeSet<i64>>) {
    for bond in spec.bonds() {
        let Some(target) = row.int(bond.name()) else {
            continue;
        };
        assert!(
            live.get(bond.target())
                .is_some_and(|keys| keys.contains(&target)),
            "{} {} refers through {} to ended {} {target}",
            spec.name(),
            row.key(),
            bond.name(),
            bond.target()
        );
    }
}

async fn settled(store: &Store, strand: &str) {
    store
        .reconcile_effects(&format!("turn_{strand}"), LATER)
        .await
        .expect("settle prepared effect");
    store
        .transition_job(
            &format!("job_{strand}"),
            santi_estate::TransitionDraft::new(santi_model::job::State::Succeeded, LATER),
        )
        .await
        .expect("complete job");
    store
        .acknowledge_job(&format!("job_{strand}"), LATER)
        .await
        .expect("acknowledge job");
    let santi_estate::Opening::Started(started) = store
        .drain_turn(santi_estate::DrainDraft {
            turn: &format!("drain_{strand}"),
            strand,
            trigger: santi_model::turn::Trigger::StrandSend,
            actor: "santi",
            created: LATER,
        })
        .await
        .expect("drain pending input")
    else {
        panic!("expected pending input");
    };
    store
        .finish_turn(santi_estate::CompletionDraft {
            turn: &started.turn.id,
            reply: None,
            provider: "test",
            model: "test",
            response: None,
            occurred: LATER,
        })
        .await
        .expect("complete drained input");
}

mod guards;
mod race;
