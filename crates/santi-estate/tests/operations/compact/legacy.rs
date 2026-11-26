use super::*;
use keel::adapt::db::Sqlite;
use keel::{Core, Op, form};

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn graph() -> keel::Graph {
    use keel::{
        atom::Kind,
        bond::Kind as Bond,
        spec::{Only, Spec},
    };
    let mut graph = keel::Graph::new();
    for spec in santi_estate::graph().nodes().values() {
        if spec.name() != "Compact" {
            graph.add(spec.clone());
        }
    }
    graph.add(
        Spec::build("Compact")
            .sole("tag", Kind::Text)
            .field("summary", Kind::Text)
            .optional("created", Kind::Text, Only::Free)
            .optional("metadata", Kind::Text, Only::Free)
            .root("strand", Bond::Many2one, "Strand")
            .bond("first", Bond::Many2one, "Message", &[])
            .bond("last", Bond::Many2one, "Message", &[])
            .freeze()
            .seal(),
    );
    graph
}

async fn seed(core: &Core<Sqlite>) -> i64 {
    let soul = core
        .put(
            "Soul",
            &[("tag", "soul_test"), ("created", FIRST), ("updated", FIRST)],
        )
        .await
        .unwrap();
    let strand = core
        .put(
            "Strand",
            &[
                ("tag", "strand_test"),
                ("soul", &soul.to_string()),
                ("next", "2"),
                ("created", FIRST),
                ("updated", FIRST),
            ],
        )
        .await
        .unwrap();
    let content =
        serde_json::to_string(&message::Content::text("original legacy content")).unwrap();
    let message = core
        .put(
            "Message",
            &[
                ("tag", "message_legacy"),
                ("strand", &strand.to_string()),
                ("actor_type", "soul"),
                ("actor", "soul_test"),
                ("content", &content),
                ("state", "fixed"),
                ("created", FIRST),
                ("updated", FIRST),
            ],
        )
        .await
        .unwrap();
    core.put(
        "StrandEntry",
        &[
            ("strand", &strand.to_string()),
            ("message", &message.to_string()),
            ("target_type", "message"),
            ("sequence", "1"),
            ("created", FIRST),
        ],
    )
    .await
    .unwrap();
    core.put(
        "Compact",
        &[
            ("tag", "compact_legacy"),
            ("strand", &strand.to_string()),
            ("first", &message.to_string()),
            ("last", &message.to_string()),
            ("summary", "legacy summary"),
            ("created", FIRST),
        ],
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn preserves() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let wire = Sqlite::file(&path).await.unwrap();
    let core = keel::bootstrap(graph(), wire)
        .unwrap()
        .seal(SUDO)
        .await
        .unwrap()
        .share();
    let key = seed(&core).await;
    drop(core);
    let store = Store::open(&path)
        .await
        .expect("Keel evolves old estate without replacing Compact identity");
    let check = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
        .await
        .unwrap()
        .share();
    let retained = check
        .one(&form("Compact").when("id", Op::Eq, &key.to_string()))
        .await
        .unwrap()
        .expect("legacy key retained through schema evolution");
    assert_eq!(retained.text("tag"), Some("compact_legacy"));
    assert!(retained.int("head").is_none() && retained.int("first").is_some());
    drop(check);
    let compact = store.compact("compact_legacy").await.unwrap().unwrap();
    assert_eq!(
        (compact.first.as_str(), compact.last.as_str()),
        ("message_legacy", "message_legacy")
    );
    let page = store.page(&compact.id, None, 0, 10).await.unwrap().unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].text, "original legacy content");
    let child = store
        .fork(santi_estate::Fork {
            tag: "strand_child",
            parent: "strand_test",
            at: 1,
            inference: None,
            created: LATER,
        })
        .await
        .unwrap();
    assert_eq!(store.compacts(&child.id).await.unwrap().len(), 1);
    store
        .condense(Compact {
            tag: "compact_rewrite",
            strand: "strand_test",
            first: "message_legacy",
            last: "message_legacy",
            summary: "new summary over the same original",
            metadata: None,
            expected: None,
            created: LATER,
        })
        .await
        .unwrap();
    drop(store);
    let core = keel::bind(santi_estate::graph(), Sqlite::file(&path).await.unwrap())
        .await
        .unwrap()
        .share();
    let legacy = core
        .one(&form("Compact").when("id", Op::Eq, &key.to_string()))
        .await
        .unwrap();
    assert!(legacy.is_none(), "absorbed legacy compact retired normally");
    drop(core);
    let reopened = Store::open(&path).await.unwrap();
    let page = reopened
        .page("compact_rewrite", None, 0, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(page.entries[0].text, "original legacy content");
}
