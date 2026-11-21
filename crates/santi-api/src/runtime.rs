use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use crate::config::{Layout, Profile};
use santi_core::inference::Declaration;
use santi_core::soul;

#[derive(Debug)]
pub struct Runtime {
    pub operator: Option<PathBuf>,
    pub bind: String,
    pub port: u16,
    pub inference: Declaration,
    pub custody: BTreeMap<String, Profile>,
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
    pub feedback: Option<Feedback>,
    pub secret: Option<String>,
    pub allow: Option<String>,
}

static HELD: OnceLock<Runtime> = OnceLock::new();

pub fn hold(runtime: Runtime) {
    let _ = HELD.set(runtime);
}

pub fn operator() -> Option<&'static std::path::Path> {
    HELD.get()?.operator.as_deref()
}

pub fn held() -> &'static Runtime {
    HELD.get()
        .expect("the runtime is resolved once at boot, before any use")
}

#[derive(Debug)]
pub struct Feedback {
    pub profile: String,
    pub app: String,
}

impl Feedback {
    pub fn new(profile: String, app: String) -> Result<Self, String> {
        if profile.is_empty()
            || profile.len() > 64
            || !profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("invalid Feishu feedback profile".into());
        }
        if !app.starts_with("cli_")
            || app.len() > 128
            || !app.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid Feishu feedback App ID".into());
        }
        Ok(Self { profile, app })
    }
}
