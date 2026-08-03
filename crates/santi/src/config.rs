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
