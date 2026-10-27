use super::Service;
use crate::assembly::prompt::{Prompting, prompted};
use crate::{now, render, strand::Strand, stream};

pub(in crate::service) type Key = (String, render::Kind);
pub(in crate::service) const PLAIN: &str = "text/plain; charset=utf-8";

impl Service {
    pub async fn render(
        &self,
        strand: &str,
        request: render::Request,
    ) -> Result<render::View, String> {
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        match request.kind {
            render::Kind::SystemPrompt => {
                let mut rendered = self.prompt(&strand)?;
                rendered.text = self.composition(&strand.id).await?.text;
                Ok(self.cached(rendered))
            }
            render::Kind::Skill => Ok(self.cached(render::View {
                strand: strand.id,
                kind: render::Kind::Skill,
                content_type: "text/markdown; charset=utf-8".into(),
                text: crate::assembly::skill::TEXT.into(),
                updated: now(),
            })),
        }
    }

    pub(in crate::service) fn prompt(&self, strand: &Strand) -> Result<render::View, String> {
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
        Ok(render::View {
            strand: id.to_string(),
            kind: render::Kind::SystemPrompt,
            content_type: PLAIN.to_string(),
            text,
            updated: now(),
        })
    }

    pub(in crate::service) fn cached(&self, view: render::View) -> render::View {
        let key: Key = (view.strand.clone(), view.kind.clone());
        let mut cache = self.renders.lock().unwrap();
        if let Some(existing) = cache.get(&key)
            && existing.text == view.text
            && existing.content_type == view.content_type
        {
            return existing.clone();
        }
        cache.insert(key, view.clone());
        drop(cache);
        self.publish(
            &view.strand,
            stream::Payload::Render(crate::render::Beat::Updated {
                view: render::Updated {
                    strand: view.strand.clone(),
                    kind: view.kind.clone(),
                    updated: view.updated.clone(),
                },
            }),
        );
        view
    }
}
