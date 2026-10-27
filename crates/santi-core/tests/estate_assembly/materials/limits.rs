use super::*;

#[test]
fn refuses() {
    let root = tempfile::tempdir().unwrap();
    skill(root.path(), "plumb");
    let library = catalog(root.path(), Format::Skill);
    let path = root.path().join("plumb/SKILL.md");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.clone() + &"x".repeat(262144)).unwrap();
    assert!(
        library
            .resolve("perish:plumb")
            .unwrap_err()
            .contains("exceeds")
    );
    fs::write(&path, text).unwrap();
    fs::write(root.path().join("plumb/metadata.json"), "{}").unwrap();
    assert!(
        library
            .resolve("perish:plumb")
            .unwrap_err()
            .contains("metadata invalid")
    );
    fs::write(&path, "---\nname: plumb\n").unwrap();
    assert!(
        library
            .resolve("perish:plumb")
            .unwrap_err()
            .contains("incomplete")
    );
}

#[test]
fn builtin() {
    let a = Catalog::default().resolve("builtin:santi").unwrap();
    let b = Catalog::default().resolve("builtin:santi").unwrap();
    assert_eq!(a, b);
    assert!(a.contents[0].text.contains("# Santi"));
    assert!(
        Catalog::new(
            [(
                "builtin".into(),
                Source {
                    root: "/tmp".into(),
                    format: Format::Skill
                }
            )]
            .into()
        )
        .is_err()
    );
    assert!(
        Catalog::new(
            [(
                "x".into(),
                Source {
                    root: "relative".into(),
                    format: Format::Native
                }
            )]
            .into()
        )
        .is_err()
    );
}
