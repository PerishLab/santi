use super::{Blob, Package};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
struct Identity<'a> {
    name: &'a str,
    description: &'a str,
    entry: &'a [String],
    blobs: &'a [Blob],
}

fn seal(mut package: Package) -> Result<Package, String> {
    package.contents.sort_by(|a, b| a.name.cmp(&b.name));
    for count in [package.contents.len(), package.material.entry.len()] {
        if !(1..=32).contains(&count) {
            return Err("material requires 1..32 blobs and entry names".into());
        }
    }
    if package.material.name.trim().is_empty() {
        return Err("material requires 1..32 blobs and entry names".into());
    }
    let mut names = std::collections::BTreeSet::new();
    for content in &package.contents {
        if content.name.is_empty() || !names.insert(&content.name) || content.text.len() > 262_144 {
            return Err("material blob requires unique name and at most 262144 bytes".into());
        }
        if !matches!(
            content.mime.as_str(),
            "text/plain"
                | "text/markdown"
                | "text/plain; charset=utf-8"
                | "text/markdown; charset=utf-8"
        ) {
            return Err("material blob requires a supported text MIME type".into());
        }
    }
    let bytes: usize = package.contents.iter().map(|c| c.text.len()).sum();
    if bytes > 1_048_576 {
        return Err("material package exceeds 1048576 bytes".into());
    }
    if package.material.description.trim().is_empty() || package.material.description.len() > 8192 {
        return Err("material description requires 1..8192 bytes".into());
    }
    let mut entries = std::collections::BTreeSet::new();
    for entry in &package.material.entry {
        if !entries.insert(entry)
            || !package
                .contents
                .iter()
                .any(|c| &c.name == entry && !c.text.trim().is_empty())
        {
            return Err("material entry must name a unique nonempty blob".into());
        }
    }
    package.material.blobs = package
        .contents
        .iter()
        .map(|c| Blob {
            name: c.name.clone(),
            mime: c.mime.clone(),
            bytes: c.text.len() as u64,
            digest: format!("{:x}", Sha256::digest(c.text.as_bytes())),
        })
        .collect();
    let identity = Identity {
        name: &package.material.name,
        description: &package.material.description,
        entry: &package.material.entry,
        blobs: &package.material.blobs,
    };
    package.material.revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&identity).map_err(|e| e.to_string())?)
    );
    Ok(package)
}

impl Package {
    pub fn sealed(self) -> Result<Self, String> {
        seal(self)
    }

    pub fn verify(&self) -> Result<(), String> {
        let checked = self.clone().sealed()?;
        if checked.material.revision != self.material.revision
            || checked.material.blobs != self.material.blobs
        {
            return Err("material package disagrees with content revision".into());
        }
        Ok(())
    }
}
