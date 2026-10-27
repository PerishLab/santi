use super::{NOW, SUDO};
use keel::adapt::db::Sqlite;
use keel::atom::Kind;
use keel::bond;
use keel::spec::{Rule, Spec};
use santi_estate::{Store, object};

fn predecessor() -> keel::Graph {
    let mut graph = keel::Graph::new();
    for (key, spec) in santi_estate::graph().nodes() {
        if key != "strand:object" {
            graph.add(spec.clone());
        }
    }
    graph.add(
        Spec::build("Material")
            .per("key", Kind::Text, &["strand"])
            .field("length", Kind::Int)
            .rule("length", Rule::new().min(0))
            .field("sha256", Kind::Text)
            .field("created", Kind::Text)
            .field("updated", Kind::Text)
            .root("strand", bond::Kind::Many2one, "Strand")
            .seal(),
    );
    graph
}

#[tokio::test]
async fn objects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("estate");
    let core = keel::bootstrap(predecessor(), Sqlite::file(&path).await.unwrap())
        .unwrap()
        .seal(SUDO)
        .await
        .unwrap();
    core.put(
        "Soul",
        &[("tag", "soul"), ("created", NOW), ("updated", NOW)],
    )
    .await
    .unwrap();
    core.put(
        "Strand",
        &[
            ("tag", "strand"),
            ("soul", "1"),
            ("created", NOW),
            ("updated", NOW),
        ],
    )
    .await
    .unwrap();
    for key in ["reports/a.txt", "retired.txt"] {
        core.put(
            "Material",
            &[
                ("key", key),
                ("length", "3"),
                ("sha256", "digest"),
                ("created", NOW),
                ("updated", NOW),
                ("strand", "1"),
            ],
        )
        .await
        .unwrap();
    }
    core.end("Material", 2).await.unwrap();
    let before = core.live("Material").await.unwrap();
    drop(core);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(
        store.objects("strand").await.unwrap(),
        vec![object::Object {
            strand: "strand".into(),
            key: "reports/a.txt".into(),
            length: 3,
            sha256: "digest".into(),
            created: NOW.into(),
            updated: NOW.into(),
        }]
    );
    assert_eq!(store.strand("strand").await.unwrap().unwrap().soul, "soul");
    drop(store);
    let core = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
        .await
        .unwrap();
    assert_eq!(core.live("Object").await.unwrap(), before);
    assert!(core.live("Material").await.is_err());
    assert!(core.seal(SUDO).await.unwrap());
    drop(core);
    let store = Store::open(&path).await.unwrap();
    store
        .record(object::Draft {
            strand: "strand",
            key: "new.txt",
            length: 4,
            sha256: "new digest",
            occurred: NOW,
        })
        .await
        .unwrap();
    assert!(store.erase("strand", "reports/a.txt").await.unwrap());
    assert_eq!(store.objects("strand").await.unwrap()[0].key, "new.txt");
    drop(store);
    let core = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
        .await
        .unwrap();
    assert_eq!(core.live("Object").await.unwrap()[0].key(), 3);
}
