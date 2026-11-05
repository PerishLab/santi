use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use crate::config::{Layout, Profile, Providers, Resolved};
use santi_core::inference::{Declaration, Effective, Origin};
use santi_core::soul;

#[derive(Debug)]
pub struct Runtime {
    pub bind: String,
    pub port: u16,
    pub inference: Declaration,
    pub providers: BTreeMap<String, Profile>,
    pub environment: BTreeMap<String, String>,
    pub paths: Layout,
    pub grace: Duration,
    pub retention: Duration,
    pub github: Github,
    pub feishu: Feishu,
    pub capability: Option<santi_core::capability::Issuer>,
    pub execution: santi_core::budget::Execution,
    pub constitution: Option<PathBuf>,
    pub memory: usize,
    pub materials: santi_core::material::Sources,
    pub souls: BTreeMap<String, soul::Identity>,
}

#[derive(Debug, Default)]
pub struct Github {
    pub login: Option<String>,
    pub allow: Option<String>,
}

#[derive(Debug, Default)]
pub struct Feishu {
    pub acknowledge: Option<String>,
    pub secret: Option<String>,
    pub allow: Option<String>,
}

impl Runtime {
    pub fn provider(&self) -> &str {
        self.inference.provider.as_deref().unwrap_or_default()
    }

    fn base(&self) -> Result<Resolved, String> {
        self.inference.validate()?;
        let provider = self.provider();
        let profile = self
            .providers
            .get(provider)
            .ok_or_else(|| format!("provider {provider} is not defined in the config"))?;
        profile.resolve(provider)
    }

    pub fn effective(&self) -> Result<Effective, String> {
        self.base()?;
        Effective::resolve(
            &[(&self.inference, Origin::Service)],
            &Providers(&self.providers),
        )
    }

    pub fn resolved(&self) -> Result<Resolved, String> {
        let base = self.base()?;
        let effective = self.effective()?;
        base.rebase(&effective)
    }
}

static HELD: OnceLock<Runtime> = OnceLock::new();

pub fn hold(runtime: Runtime) {
    let _ = HELD.set(runtime);
}

pub fn held() -> &'static Runtime {
    HELD.get()
        .expect("the runtime is resolved once at boot, before any use")
}
