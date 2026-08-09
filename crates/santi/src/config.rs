use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct Client {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub auth_token_url: Option<String>,
    pub auth_client_id: Option<String>,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct Names {
    #[serde(default)]
    pub alias: HashMap<String, String>,
}

pub fn names() -> Names {
    let Some(home) = env("HOME") else {
        return Names::default();
    };
    let _ = home;
    let Ok(path) = alias() else {
        return Names::default();
    };
    if !path.is_file() {
        return Names::default();
    }
    plumb::config::load(&path).unwrap_or_default()
}

pub fn rename(id: &str, name: &str) -> Result<PathBuf> {
    let path = alias()?;
    let mut names = names();
    if name.is_empty() {
        names.alias.remove(id);
    } else {
        names.alias.insert(id.to_string(), name.to_string());
    }
    let mut body = String::from("[alias]\n");
    let mut sorted = names.alias.into_iter().collect::<Vec<_>>();
    sorted.sort();
    for (held, called) in sorted {
        body.push_str(&format!("{held:?} = {called:?}\n"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&path, body).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

fn alias() -> Result<PathBuf> {
    let home = env("HOME").context("HOME is unset")?;
    Ok(env("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&home).join(".config"))
        .join("santi/alias.toml"))
}

pub fn load() {
    dotenvy::dotenv().ok();
}

pub fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn shelter() -> PathBuf {
    env("HOME")
        .map(|home| PathBuf::from(home).join(".cache/santi"))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn client() -> Result<Client> {
    let cwd = std::env::current_dir().context("read current directory")?;
    let Ok(product) = plumb::config::discover(&cwd, "plumb.toml") else {
        return Ok(Client::default());
    };
    let root = product.parent().context("plumb.toml has no parent")?;
    let path = root.join(".local/secrets/santi.toml");
    if !path.is_file() {
        return Ok(Client::default());
    }
    plumb::config::load(&path).with_context(|| format!("load {}", path.display()))
}

pub struct Resume<'a> {
    pub base: &'a str,
    pub soul: &'a str,
    pub strand: &'a str,
    pub bearer: Option<&'a str>,
}

pub fn resume(command: &mut std::process::Command, resume: Resume<'_>) {
    command
        .env("SANTI_API_KEY", resume.bearer.unwrap_or_default())
        .env("SANTI_AUTH_TOKEN_URL", "")
        .env("SANTI_AUTH_CLIENT_ID", "")
        .env("SANTI_AUTH_USERNAME", "")
        .env("SANTI_AUTH_PASSWORD", "")
        .env("SANTI_API_URL", resume.base)
        .env("SANTI_SOUL_ID", resume.soul)
        .env("SANTI_STRAND_ID", resume.strand);
}

pub fn executable() -> Result<PathBuf> {
    std::env::current_exe().context("resolve the current executable")
}
