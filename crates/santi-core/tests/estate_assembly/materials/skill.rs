use super::*;

#[test]
fn revisions() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    skill(a.path(), "plumb");
    skill(b.path(), "plumb");
    let first = catalog(a.path(), Format::Skill)
        .resolve("perish:plumb")
        .unwrap();
    let second = catalog(b.path(), Format::Skill)
        .resolve("perish:plumb")
        .unwrap();
    assert_eq!(first.material.revision, second.material.revision);
    assert_eq!(first.contents.len(), 1);
    assert_eq!(first.material.provenance.version.as_deref(), Some("v1.2.3"));
    assert!(!first.contents[0].text.contains("keeper"));
    let p = b.path().join("plumb/metadata.json");
    let mut m: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
    m["version"] = "v1.2.4".into();
    fs::write(p, m.to_string()).unwrap();
    let newer = catalog(b.path(), Format::Skill)
        .resolve("perish:plumb")
        .unwrap();
    assert_eq!(first.material.revision, newer.material.revision);
    assert_ne!(
        first.material.provenance.version,
        newer.material.provenance.version
    );
    fs::write(
        b.path().join("plumb/SKILL.md"),
        second.contents[0].text.clone() + "Changed.\n",
    )
    .unwrap();
    assert_ne!(
        first.material.revision,
        catalog(b.path(), Format::Skill)
            .resolve("perish:plumb")
            .unwrap()
            .material
            .revision
    );
}

#[test]
fn discovery() {
    let root = tempfile::tempdir().unwrap();
    skill(root.path(), "plumb");
    skill(root.path(), "concord");
    let catalog = catalog(root.path(), Format::Skill);
    let View::Page(page) = catalog
        .query(Query {
            available: true,
            limit: 2,
            ..Default::default()
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.next, Some(2));
    assert_eq!(page.items[0].reference, "builtin:santi");
    assert_eq!(page.items[1].reference, "perish:concord");
    let View::Blob(blob) = catalog
        .query(Query {
            reference: Some("perish:plumb".into()),
            blob: Some("SKILL.md".into()),
            ..Default::default()
        })
        .unwrap()
    else {
        panic!()
    };
    assert!(blob.text.contains("Use current authority"));
    assert!(catalog.resolve("perish:missing").is_err());
    assert!(
        catalog
            .query(Query {
                available: true,
                limit: 101,
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        catalog
            .query(Query {
                available: true,
                offset: 99,
                ..Default::default()
            })
            .is_ok()
    );
}
