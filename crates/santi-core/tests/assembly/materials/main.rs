use santi_core::{
    library::Catalog,
    material::{Format, Query, Source, View},
};
use std::{fs, path::Path};

mod limits;
mod native;
mod skill;

fn catalog(root: &Path, format: Format) -> Catalog {
    Catalog::new(
        [(
            "perish".into(),
            Source {
                root: root.display().to_string(),
                format,
            },
        )]
        .into(),
    )
    .unwrap()
}

fn skill(root: &Path, id: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {id}\ndescription: Operate {id}.\n---\n\n# Guide\nUse current authority.\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("metadata.json"),
        serde_json::json!({"schema":1,"name":id,"version":"v1.2.3","keeper":id}).to_string(),
    )
    .unwrap();
}
