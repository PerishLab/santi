use super::super::Service;
use crate::{environ::Scope, material};

pub(in crate::service) struct Composition {
    pub text: String,
    pub rendered: String,
    pub selection: material::Selection,
    pub components: Vec<material::Component>,
}

impl Composition {
    pub(in crate::service) async fn capture(
        &self,
        store: &santi_estate::Store,
        turn: &str,
        round: usize,
    ) -> Result<(), String> {
        store
            .capture(santi_estate::Capture {
                tag: &crate::tag("context"),
                turn,
                round,
                selection: &self.selection,
                prompt: &self.text,
                rendered: &self.rendered,
                components: &self.components,
                created: &crate::now(),
            })
            .await
            .map(|_| ())
    }

    pub(super) fn new(base: String, selection: material::Selection) -> Result<Self, String> {
        let mut rendered = String::new();
        let mut components = Vec::new();
        for package in &selection.packages {
            package.verify()?;
            let metadata = &package.material;
            let mut fragment = format!(
                "\n<material>\nreference: {}\nrevision: {}\nname: {}\ndescription: {}\n",
                metadata.reference, metadata.revision, metadata.name, metadata.description
            );
            for entry in &metadata.entry {
                let content = package
                    .contents
                    .iter()
                    .find(|c| &c.name == entry)
                    .ok_or_else(|| "material entry missing from revision".to_string())?;
                fragment.push_str(&format!(
                    "\n<blob name={entry:?}>\n{}\n</blob>\n",
                    content.text
                ));
            }
            fragment.push_str("</material>\n");
            components.push(material::Component {
                reference: metadata.reference.clone(),
                revision: metadata.revision.clone(),
                entry: metadata.entry.clone(),
                bytes: fragment.len(),
                origins: selection
                    .bindings
                    .iter()
                    .filter(|b| {
                        b.reference == metadata.reference && b.revision == metadata.revision
                    })
                    .map(|b| material::Origin {
                        scope: b.scope,
                        provenance: b.provenance.clone(),
                    })
                    .collect(),
            });
            rendered.push_str(&fragment);
        }
        Ok(Self {
            text: format!("{base}{rendered}"),
            rendered,
            selection,
            components,
        })
    }
}

impl Service {
    pub(in crate::service) async fn composition(
        &self,
        strand: &str,
    ) -> Result<Composition, String> {
        let brain = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let selection = self.store.bindings(Scope::Strand, strand).await?;
        let composed = Composition::new(self.prompt(&brain)?.text, selection)?;
        self.cached(material::Material {
            strand: strand.into(),
            kind: material::Kind::SystemPrompt,
            content_type: super::PLAIN.into(),
            text: composed.text.clone(),
            updated: crate::now(),
        });
        Ok(composed)
    }
}
