pub(in crate::service) mod address;
pub(in crate::service) mod drive;
pub(in crate::service) mod error;
pub(in crate::service) mod materials;
pub(in crate::service) mod notice;
pub(in crate::service) mod render;
pub(in crate::service) mod thinking;
pub(in crate::service) mod timing;
pub(in crate::service) mod watch;

use super::Service;

pub mod provider {
    use std::sync::Arc;

    use santi_provider::Provider;

    use crate::inference::{Declaration, Effective, Origin, Source};

    pub trait Resolver: Send + Sync {
        fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String>;
        fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String>;
    }

    pub(crate) struct Fixed {
        provider: Arc<dyn Provider>,
        effective: Effective,
    }

    impl Fixed {
        pub(crate) fn new(provider: Arc<dyn Provider>) -> Self {
            let metadata = provider.metadata();
            let effective = Effective {
                provider: metadata.provider.to_string(),
                model: metadata.model,
                reasoning_effort: None,
                source: Source {
                    provider: Origin::Default,
                    model: Origin::Default,
                    reasoning_effort: None,
                },
            };
            Self {
                provider,
                effective,
            }
        }
    }

    impl Resolver for Fixed {
        fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
            if layers.iter().any(|(declaration, _)| !declaration.empty()) {
                return Err(
                    "fixed provider cannot honor Soul or Strand inference declarations; configure a provider directory resolver"
                        .to_string(),
                );
            }
            Ok(self.effective.clone())
        }

        fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
            if effective != &self.effective {
                return Err(
                    "fixed provider cannot build a different inference selection".to_string(),
                );
            }
            Ok(self.provider.clone())
        }
    }
}
