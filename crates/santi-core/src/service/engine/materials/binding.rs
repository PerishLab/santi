use super::{Service, composition::Composition};
use crate::{environ::Scope, material};

impl Service {
    pub(super) async fn bound(
        &self,
        scope: Scope,
        owner: &str,
        query: material::Query,
    ) -> Result<material::View, String> {
        if let Some(operation) = query.operation.as_deref() {
            return self
                .store
                .operation(scope, owner, operation)
                .await?
                .map(material::View::Change)
                .ok_or_else(|| "material operation not found".into());
        }
        if let Some(context) = query.context.as_deref() {
            if scope != Scope::Strand {
                return Err("material context requires a strand".into());
            }
            if context == "all" {
                return Ok(material::View::Contexts {
                    contexts: self.store.contexts(owner).await?,
                });
            }
            let observed = self
                .store
                .context(owner, context)
                .await?
                .ok_or_else(|| "material context not found".to_string())?;
            if let Some(reference) = query.reference.as_deref() {
                let packages = self.store.archived(owner, context).await?;
                return inspect(&packages, reference, query.blob.as_deref());
            }
            return Ok(material::View::Context(observed));
        }
        if query.available {
            return self.catalog(query);
        }
        let selection = self.store.bindings(scope, owner).await?;
        if let Some(reference) = query.reference.as_deref() {
            if selection
                .packages
                .iter()
                .any(|p| p.material.reference == reference)
            {
                return inspect(&selection.packages, reference, query.blob.as_deref());
            }
            return self.catalog(query);
        }
        if query.blob.is_some() {
            return Err("material blob requires a reference".into());
        }
        Ok(material::View::State(state(selection, query.effective)))
    }

    #[radicula_lock::guard(gate = self.binding.tokio(), error = |failure: radicula_lock::Failure<_>| Err(failure.to_string()))]
    pub async fn load(
        &self,
        scope: Scope,
        owner: &str,
        command: material::Command,
    ) -> Result<material::Applied, String> {
        let package = self.library.resolve(&command.reference)?;
        self.admissible(scope, owner, &package).await?;
        let operation = command.operation.unwrap_or_else(|| crate::tag("mo"));
        let change = self
            .store
            .bind(santi_estate::Bind {
                scope,
                owner,
                package: &package,
                operation: &operation,
                created: &crate::now(),
            })
            .await?;
        self.applied(scope, owner, change).await
    }

    #[radicula_lock::guard(gate = self.binding.tokio(), error = |failure: radicula_lock::Failure<_>| Err(failure.to_string()))]
    pub async fn drop(
        &self,
        scope: Scope,
        owner: &str,
        command: material::Command,
    ) -> Result<material::Applied, String> {
        let operation = command.operation.unwrap_or_else(|| crate::tag("mo"));
        let change = self
            .store
            .unbind(santi_estate::Unbind {
                scope,
                owner,
                reference: &command.reference,
                operation: &operation,
                created: &crate::now(),
            })
            .await?;
        self.applied(scope, owner, change).await
    }

    async fn applied(
        &self,
        scope: Scope,
        owner: &str,
        change: material::Change,
    ) -> Result<material::Applied, String> {
        let state = state(self.store.bindings(scope, owner).await?, false);
        let estimate = if scope == Scope::Strand {
            self.estimate(owner).await.ok()
        } else {
            None
        };
        Ok(material::Applied { change, state, estimate, boundary: "next provider round; in-flight requests and recorded history retain their selection".into() })
    }

    async fn admissible(
        &self,
        scope: Scope,
        owner: &str,
        package: &material::Package,
    ) -> Result<(), String> {
        self.store.bindings(scope, owner).await?;
        let strands = if scope == Scope::Strand {
            vec![
                self.store
                    .strand(owner)
                    .await?
                    .ok_or_else(|| "strand not found".to_string())?,
            ]
        } else {
            self.store
                .strands()
                .await?
                .into_iter()
                .filter(|s| s.soul == owner)
                .collect()
        };
        for strand in strands {
            let mut selection = self.store.bindings(Scope::Strand, &strand.id).await?;
            selection
                .packages
                .retain(|p| p.material.reference != package.material.reference);
            selection.packages.push(package.clone());
            selection
                .packages
                .sort_by(|a, b| a.material.reference.cmp(&b.material.reference));
            let composed = Composition::new(self.prompt(&strand)?.text, selection)?;
            self.fixed(&composed.text)?;
        }
        Ok(())
    }
    fn fixed(&self, text: &str) -> Result<(), String> {
        let Some(cap) = self.budget() else {
            return Ok(());
        };
        for (tools, ceiling) in [
            (crate::service::tools::tools(), cap.bytes),
            (crate::service::tools::maintenance(), cap.ceiling()),
        ] {
            let request = self.projected(Vec::new(), text.into(), tools)?;
            let estimate = crate::context::budget::gauged(&request, self.provider.as_ref());
            if estimate.total > ceiling {
                return Err(format!(
                    "material fixed context exceeds budget ({} > {}); drop material before loading more",
                    estimate.total, ceiling
                ));
            }
        }
        Ok(())
    }
}

fn state(selection: material::Selection, all: bool) -> material::State {
    let bindings = selection
        .bindings
        .iter()
        .filter(|b| all || b.scope == selection.scope)
        .cloned()
        .collect();
    let mut effective = Vec::<material::Binding>::new();
    for binding in selection.bindings {
        if !effective.iter().any(|b| b.reference == binding.reference) {
            effective.push(binding);
        }
    }
    material::State {
        scope: selection.scope,
        owner: selection.owner,
        bindings,
        effective,
    }
}

fn inspect(
    packages: &[material::Package],
    reference: &str,
    blob: Option<&str>,
) -> Result<material::View, String> {
    let package = packages
        .iter()
        .find(|p| p.material.reference == reference)
        .ok_or_else(|| "material reference not found in context".to_string())?;
    match blob {
        Some(name) => package
            .contents
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .map(material::View::Blob)
            .ok_or_else(|| "material blob not found".into()),
        None => Ok(material::View::Material(package.material.clone())),
    }
}
