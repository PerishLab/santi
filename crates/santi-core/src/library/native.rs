use super::read;
use crate::material::{Content, Descriptor, Package, Provenance};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    description: String,
    entry: Vec<String>,
    blobs: BTreeMap<String, File>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    mime: String,
}

pub(super) fn package(root: &read::Root, reference: &str) -> Result<Package, String> {
    let text = root.text("material.toml")?;
    let manifest: Manifest =
        toml::from_str(&text).map_err(|e| format!("material manifest invalid: {e}"))?;
    read::name(&manifest.name)?;
    if manifest.entry.is_empty() || manifest.blobs.is_empty() || manifest.blobs.len() > 32 {
        return Err("material requires entries and 1..32 blobs".into());
    }
    let mut contents = Vec::new();
    for (name, file) in manifest.blobs {
        read::relative(&name)?;
        if !matches!(file.mime.as_str(), "text/plain" | "text/markdown") {
            return Err("material supports text/plain and text/markdown blobs".into());
        }
        contents.push(Content {
            name,
            mime: file.mime,
            text: root.text(&file.path)?,
        });
    }
    Ok(Package {
        material: Descriptor {
            reference: reference.into(),
            name: manifest.name,
            description: manifest.description,
            revision: String::new(),
            entry: manifest.entry,
            blobs: Vec::new(),
            provenance: Provenance {
                source: reference.split_once(':').unwrap().0.into(),
                format: "native".into(),
                version: None,
                keeper: None,
            },
        },
        contents,
    })
}
