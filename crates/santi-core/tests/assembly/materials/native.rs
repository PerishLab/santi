use super::*;

fn prepare(root: &Path) {
    let dir = root.join("writing");
    fs::create_dir(&dir).unwrap();
    fs::write(
        dir.join("material.toml"),
        r#"name="writing"
description="Writing methods"
entry=["guide"]
[blobs.guide]
path="guide.md"
mime="text/markdown"
[blobs.reference]
path="reference.txt"
mime="text/plain"
"#,
    )
    .unwrap();
    fs::write(dir.join("guide.md"), "Read selectively.").unwrap();
    fs::write(dir.join("reference.txt"), "Other evidence.").unwrap();
}

#[test]
fn package() {
    let root = tempfile::tempdir().unwrap();
    prepare(root.path());
    let package = catalog(root.path(), Format::Native)
        .resolve("perish:writing")
        .unwrap();
    assert_eq!(package.contents.len(), 2);
    assert_eq!(package.material.entry, vec!["guide"]);
    assert_eq!(package.material.blobs[0].bytes, 17);
    fs::remove_file(root.path().join("writing/reference.txt")).unwrap();
    assert!(
        catalog(root.path(), Format::Native)
            .resolve("perish:writing")
            .is_err()
    );
}

#[test]
fn paths() {
    let root = tempfile::tempdir().unwrap();
    prepare(root.path());
    let path = root.path().join("writing/material.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(path, text.replace("guide.md", "../outside.md")).unwrap();
    assert!(
        catalog(root.path(), Format::Native)
            .resolve("perish:writing")
            .unwrap_err()
            .contains("traversal")
    );
    assert!(
        catalog(root.path(), Format::Native)
            .resolve("perish:../writing")
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    skill(outside.path(), "plumb");
    std::os::unix::fs::symlink(outside.path().join("plumb"), root.path().join("plumb")).unwrap();
    assert!(
        catalog(root.path(), Format::Skill)
            .resolve("perish:plumb")
            .unwrap_err()
            .contains("escapes")
    );
}
