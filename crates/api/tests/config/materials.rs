use super::read;

#[test]
fn sources() {
    assert!(read("").materials.is_empty());
    let root = if cfg!(windows) {
        r"C:\some\skills"
    } else {
        "/some/skills"
    };
    let text = format!("[materials.perish]\nroot='{root}'\nformat='skill'\n");
    let value = read(&text);
    assert_eq!(value.materials["perish"].root, root);
    assert_eq!(
        value.materials["perish"].format,
        santi_core::material::Format::Skill
    );
    assert!(santi_core::library::Catalog::new(value.materials).is_ok());
    let value = read("[materials.local]\nroot='relative'\n");
    assert!(santi_core::library::Catalog::new(value.materials).is_err());
}
