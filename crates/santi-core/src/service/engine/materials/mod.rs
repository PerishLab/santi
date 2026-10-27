use super::Service;
use crate::material;

mod binding;
mod composition;

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

    pub(in crate::service) async fn wording(&self, strand: &str) -> Result<String, String> {
        Ok(self.composition(strand).await?.text)
    }
}
