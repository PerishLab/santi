pub(in crate::service) mod address;
pub(in crate::service) mod drive;
pub(in crate::service) mod error;
pub(in crate::service) mod materials;
pub(in crate::service) mod notice;
pub(in crate::service) mod render;
pub(in crate::service) mod thinking;
pub(in crate::service) mod timing;
pub(in crate::service) mod watch;

use super::{Config, Service, jobs};
use crate::inference::Declaration;
use santi_provider::Provider;
use std::sync::Arc;

impl Service {
    pub async fn open(
        scenario: &str,
        config: Config,
        provider: Arc<dyn Provider>,
    ) -> Result<Self, String> {
        Self::supervised(scenario, config, provider, Arc::new(jobs::Unavailable)).await
    }

    pub async fn supervised(
        scenario: &str,
        config: Config,
        provider: Arc<dyn Provider>,
        supervisor: Arc<dyn jobs::Supervisor>,
    ) -> Result<Self, String> {
        Self::provided(
            config,
            Arc::new(crate::provider::Fixed::new(scenario, provider)),
            Declaration {
                scenario: Some(scenario.into()),
                ..Default::default()
            },
            supervisor,
        )
        .await
    }
}

pub mod provider {
    use std::sync::Arc;

    use santi_provider::Provider;

    use crate::inference::{Declaration, Effective, Origin, Selection, Source};

    pub trait Resolver: Send + Sync {
        fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String>;
        fn selecting(
            &self,
            layers: &[(&Declaration, Origin)],
            _: &[crate::sources::Record],
        ) -> Result<Effective, String> {
            self.resolve(layers)
        }
        fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String>;
    }

    pub(crate) struct Fixed {
        provider: Arc<dyn Provider>,
        effective: Effective,
    }

    impl Fixed {
        pub(crate) fn new(scenario: &str, provider: Arc<dyn Provider>) -> Self {
            let metadata = provider.metadata();
            let effective = Effective {
                selection: Selection::Scenario(scenario.into()),
                model: metadata.model,
                effort: None,
                source: Source {
                    route: None,
                    selection: Selection::Scenario(Origin::Service),
                    model: Origin::Default,
                    effort: None,
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
            let mut selected = self.effective.clone();
            for (declaration, origin) in layers {
                if declaration.effort.is_some() {
                    return Err("fixed provider cannot honor reasoning effort declarations".into());
                }
                if let Some(path) = &declaration.scenario {
                    if path != selected.selection.value() {
                        return Err("fixed provider cannot honor a different scenario".into());
                    }
                    selected.source.selection = Selection::Scenario(*origin);
                }
            }
            Ok(selected)
        }

        fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
            let mut expected = self.effective.clone();
            match &effective.selection {
                Selection::Scenario(_) => {
                    expected.source.selection =
                        Selection::Scenario(*effective.source.selection.value())
                }
                Selection::Provider(_) => {
                    expected.selection =
                        Selection::Provider(self.provider.metadata().provider.to_string());
                    expected.source.selection = Selection::Provider(Origin::Default);
                }
            }
            if effective != &expected {
                return Err(
                    "fixed provider cannot build a different inference selection".to_string(),
                );
            }
            Ok(self.provider.clone())
        }
    }
}
