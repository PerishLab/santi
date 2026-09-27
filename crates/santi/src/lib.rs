pub mod auth;
pub mod cli;
pub mod client;
pub mod config;
mod text;
pub mod watch;

use anyhow::Result;
use clap::Parser;

use auth::{Credentials, resolve_edge_bearer};
use cli::{BASE, Cli, ClientDefaults};
pub async fn run() -> Result<()> {
    config::load();
    let Cli {
        base_url,
        api_key,
        auth_token_url,
        auth_client_id,
        auth_username,
        auth_password,
        strand,
        soul,
        command,
    } = Cli::parse();
    plumb::identity::ready().map_err(anyhow::Error::msg)?;
    let client = config::client()?;
    let defaults = ClientDefaults { strand, soul };
    let bearer = resolve_edge_bearer(Credentials {
        endpoint: auth_token_url
            .as_deref()
            .or(client.auth_token_url.as_deref()),
        identity: auth_client_id
            .as_deref()
            .or(client.auth_client_id.as_deref()),
        username: auth_username.as_deref().or(client.auth_username.as_deref()),
        password: auth_password.as_deref().or(client.auth_password.as_deref()),
        key: api_key.as_deref().or(client.api_key.as_deref()),
    })
    .await?;
    let base = base_url
        .or(client.base_url)
        .unwrap_or_else(|| BASE.to_owned());
    client::run(&base, bearer.as_deref(), &defaults, command).await
}
