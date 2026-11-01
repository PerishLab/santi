use std::collections::BTreeSet;

use keel::atom::Kind as Atom;

#[derive(Debug, Clone, Copy)]
enum Class {
    Own,
    Time,
    Kind,
    Digest,
    Content,
    Provenance,
}

mod fields;
use fields::TEXT;

#[test]
fn classified() {
    let graph = santi_estate::graph();
    let mut found = BTreeSet::new();
    for (unit, spec) in graph.nodes() {
        for field in spec.fields() {
            if matches!(field.kind(), Atom::Text | Atom::Link) {
                found.insert((unit.as_str(), field.name()));
            }
        }
    }
    let classified = TEXT
        .iter()
        .map(|(unit, field, _)| (*unit, *field))
        .collect::<BTreeSet<_>>();
    let unclassified = found.difference(&classified).collect::<Vec<_>>();
    let stale = classified.difference(&found).collect::<Vec<_>>();
    assert!(
        unclassified.is_empty() && stale.is_empty(),
        "unclassified {unclassified:?}, stale {stale:?}"
    );
    assert_eq!(classified.len(), TEXT.len(), "a field is classified twice");
}

#[test]
fn rooted() {
    for (unit, spec) in santi_estate::graph().nodes() {
        let roots = spec.bonds().iter().filter(|bond| bond.root()).count();
        assert!(roots <= 1, "{unit} has {roots} roots");
    }
}
