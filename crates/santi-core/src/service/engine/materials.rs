use crate::assembly::prompt::{Prompting, prompted};
use crate::{now, strand::Strand};

use super::Service;
use crate::{material, stream};

pub(in crate::service) type Key = (String, material::Kind);

const PLAIN: &str = "text/plain; charset=utf-8";

impl Service {
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
            material::Kind::SystemPrompt => self.prompt(&strand),
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
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        Ok(self.prompt(&strand)?.text)
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
        Ok(self.cached(material::Material {
            strand: id.to_string(),
            kind: material::Kind::SystemPrompt,
            content_type: PLAIN.to_string(),
            text,
            updated: now(),
        }))
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
