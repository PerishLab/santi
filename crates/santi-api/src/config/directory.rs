use std::collections::BTreeMap;

use santi_core::inference::{Defaults, Directory};

use super::*;

#[derive(Debug)]
pub struct Providers<'a>(pub &'a BTreeMap<String, Profile>);

impl Directory for Providers<'_> {
    fn defaults(&self, provider: &str) -> Result<Defaults, String> {
        let profile = self
            .0
            .get(provider)
            .ok_or_else(|| format!("provider {provider} is not defined in the config"))?;
        let resolved = profile.resolve(provider)?;
        Ok(Defaults {
            model: resolved.model().to_string(),
            reasoning_effort: resolved.effort().map(str::to_string),
        })
    }
}
