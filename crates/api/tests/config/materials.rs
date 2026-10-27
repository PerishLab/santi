use super::read;

#[test]
fn sources() {
    assert!(read("").materials.is_empty());
    let value = read("[materials.perish]\nroot='/some/skills'\nformat='skill'\n");
    assert_eq!(value.materials["perish"].root, "/some/skills");
    assert_eq!(
        value.materials["perish"].format,
        santi_core::material::Format::Skill
    );
    assert!(santi_core::library::Catalog::new(value.materials).is_ok());
    let value = read("[materials.local]\nroot='relative'\n");
    assert!(santi_core::library::Catalog::new(value.materials).is_err());
}
