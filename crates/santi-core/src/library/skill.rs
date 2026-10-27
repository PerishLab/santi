use super::read;
use crate::material::{Content, Descriptor, Package, Provenance};
use serde::Deserialize;

#[derive(Deserialize)]
struct Header {
    name: String,
    description: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mark {
    schema: u32,
    name: String,
    version: String,
    keeper: String,
}

pub(super) fn text(text: &str, reference: &str) -> Result<Package, String> {
    let normalized = text.replace("\r\n", "\n");
    let body = normalized
        .strip_prefix("---\n")
        .ok_or("material Skill requires YAML frontmatter")?;
    let (header, _) = body
        .split_once("\n---\n")
        .ok_or("material Skill frontmatter is incomplete")?;
    let header: Header = serde_yaml_ng::from_str(header)
        .map_err(|e| format!("material Skill metadata invalid: {e}"))?;
    read::name(&header.name)?;
    Ok(Package {
        material: Descriptor {
            reference: reference.into(),
            name: header.name,
            description: header.description,
            revision: String::new(),
            entry: vec!["SKILL.md".into()],
            blobs: Vec::new(),
            provenance: Provenance {
                source: reference.split_once(':').unwrap().0.into(),
                format: "skill".into(),
                version: None,
                keeper: None,
            },
        },
        contents: vec![Content {
            name: "SKILL.md".into(),
            mime: "text/markdown".into(),
            text: text.into(),
        }],
    })
}

pub(super) fn package(root: &read::Root, reference: &str) -> Result<Package, String> {
    let mut package = text(&root.text("SKILL.md")?, reference)?;
    if root.exists("metadata.json")? {
        let mark: Mark = serde_json::from_str(&root.text("metadata.json")?)
            .map_err(|e| format!("material installed metadata invalid: {e}"))?;
        if mark.schema != 1 || mark.name != package.material.name {
            return Err("material installed metadata disagrees with Skill".into());
        }
        if mark.version.is_empty() || mark.keeper.is_empty() {
            return Err("material installed provenance is incomplete".into());
        }
        package.material.provenance.version = Some(mark.version);
        package.material.provenance.keeper = Some(mark.keeper);
    }
    Ok(package)
}
