use crate::assembly::prompt::{Prompting, prompted};
use crate::{now, strand::Strand};

use super::Service;
use crate::{material, stream};

pub(in crate::service) type Key = (String, material::Kind);

mod binding;
mod composition;

const PLAIN: &str = "text/plain; charset=utf-8";

impl Service {
    pub fn catalogued(mut self, sources: material::Sources) -> Result<Self, String> {
        self.library = std::sync::Arc::new(crate::library::Catalog::new(sources)?);
        Ok(self)
    }

    pub fn catalog(&self, query: material::Query) -> Result<material::View, String> {
        if query.context.is_some() || query.operation.is_some() || query.effective {
            return Err("material binding selectors require a soul or strand".into());
        }
        self.library.query(query)
    }

    pub async fn browse(
        &self,
        scope: crate::environ::Scope,
        owner: &str,
        query: material::Query,
    ) -> Result<material::View, String> {
        let found = match scope {
            crate::environ::Scope::Soul => self.store.soul(owner).await?.is_some(),
            crate::environ::Scope::Strand => self.store.strand(owner).await?.is_some(),
        };
        if !found {
            return Err(format!("{} not found", scope.encode()));
        }
        self.bound(scope, owner, query).await
    }

    pub async fn material(
        &self,
        strand: &str,
        request: material::Request,
    ) -> Result<material::Material, String> {
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        match request.kind {
            material::Kind::SystemPrompt => {
                let mut rendered = self.prompt(&strand)?;
                rendered.text = self.composition(&strand.id).await?.text;
                Ok(self.cached(rendered))
            }
            material::Kind::Skill => Ok(self.cached(material::Material {
                strand: strand.id,
                kind: material::Kind::Skill,
                content_type: "text/markdown; charset=utf-8".into(),
                text: crate::assembly::skill::TEXT.into(),
                updated: now(),
            })),
        }
    }

    pub(in crate::service) async fn wording(&self, strand: &str) -> Result<String, String> {
        Ok(self.composition(strand).await?.text)
    }

    fn prompt(&self, strand: &Strand) -> Result<material::Material, String> {
        let id = strand.id.as_str();
        let text = prompted(Prompting {
            id,
            identity: self.identities.get(&strand.soul),
            strand,
            constitution: self.charter(),
            memoir: self.memoir(&strand.soul),
            journal: self.journal(id),
            allowance: self.regimen().allowance,
            genesis: strand.soul == crate::GENESIS,
        })?;
        Ok(material::Material {
            strand: id.to_string(),
            kind: material::Kind::SystemPrompt,
            content_type: PLAIN.to_string(),
            text,
            updated: now(),
        })
    }

    fn cached(&self, material: material::Material) -> material::Material {
        let key: Key = (material.strand.clone(), material.kind.clone());
        let mut cache = self.materials.lock().unwrap();
        if let Some(existing) = cache.get(&key)
            && existing.text == material.text
            && existing.content_type == material.content_type
        {
            return existing.clone();
        }
        cache.insert(key, material.clone());
        drop(cache);
        self.publish(
            &material.strand,
            stream::Payload::Material(crate::material::Beat::Updated {
                material: material::Updated {
                    strand: material.strand.clone(),
                    kind: material.kind.clone(),
                    updated: material.updated.clone(),
                },
            }),
        );
        material
    }
}
