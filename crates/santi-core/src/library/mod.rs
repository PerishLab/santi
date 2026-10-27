use crate::material::{self, Format, Package, Query, Sources, View};
use std::path::Path;

mod native;
mod read;
mod revision;
mod skill;

const LIMIT: usize = 512;

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    sources: Sources,
}

impl Catalog {
    pub fn new(sources: Sources) -> Result<Self, String> {
        for (name, source) in &sources {
            read::name(name)?;
            if name == "builtin" || !Path::new(&source.root).is_absolute() {
                return Err("material source requires a non-builtin name and absolute root".into());
            }
        }
        Ok(Self { sources })
    }

    pub fn resolve(&self, reference: &str) -> Result<Package, String> {
        let first = self.acquire(reference)?;
        let second = self.acquire(reference)?;
        if first != second {
            return Err("material source changed while reading; publish the complete directory atomically and retry explicitly".into());
        }
        Ok(first)
    }

    pub fn query(&self, query: Query) -> Result<View, String> {
        if query.limit == 0 || query.limit > 100 || query.offset > LIMIT {
            return Err("material query requires limit 1..100 and offset at most 512".into());
        }
        if let Some(reference) = query.reference.as_deref() {
            let package = self.resolve(reference)?;
            return match query.blob {
                Some(name) => package
                    .contents
                    .into_iter()
                    .find(|item| item.name == name)
                    .map(View::Blob)
                    .ok_or_else(|| "material blob not found".into()),
                None => Ok(View::Material(package.material)),
            };
        }
        if query.blob.is_some() || !query.available {
            return Err(
                "material query requires --available or a reference; blob requires a reference"
                    .into(),
            );
        }
        let names = self.references()?;
        let end = query.offset.saturating_add(query.limit).min(names.len());
        let items = names
            .iter()
            .skip(query.offset)
            .take(query.limit)
            .map(|name| self.resolve(name).map(|package| package.material))
            .collect::<Result<_, _>>()?;
        Ok(View::Page(material::Page {
            items,
            next: (end < names.len()).then_some(end),
        }))
    }

    fn acquire(&self, reference: &str) -> Result<Package, String> {
        let (source, id) = reference
            .split_once(':')
            .ok_or("material reference requires source:name")?;
        read::name(source)?;
        read::name(id)?;
        if source == "builtin" {
            return self.builtin(reference, id);
        }
        let config = self
            .sources
            .get(source)
            .ok_or("material source not found")?;
        let root = read::Root::new(Path::new(&config.root))?.directory(id)?;
        let package = match config.format {
            Format::Skill => skill::package(&root, reference)?,
            Format::Native => native::package(&root, reference)?,
        };
        revision::seal(package)
    }

    fn builtin(&self, reference: &str, id: &str) -> Result<Package, String> {
        if id != "santi" {
            return Err("material not found".into());
        }
        let package = skill::text(crate::assembly::skill::TEXT, reference)?;
        revision::seal(package)
    }

    fn references(&self) -> Result<Vec<String>, String> {
        let mut names = vec!["builtin:santi".to_string()];
        for (source, config) in &self.sources {
            for name in read::Root::new(Path::new(&config.root))?.children()? {
                names.push(format!("{source}:{name}"));
                if names.len() > LIMIT {
                    return Err("material catalog exceeds 512 entries".into());
                }
            }
        }
        names.sort();
        Ok(names)
    }
}
