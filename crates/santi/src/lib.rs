pub mod auth;
pub mod cli;
pub mod client;
pub mod config;
#[path = "cli/observation.rs"]
mod observation;
mod text;
pub mod watch;

use anyhow::Result;
use clap::Parser;

use auth::{Credentials, resolve_edge_bearer};
use cli::{BASE, Cli, ClientDefaults};
use config::ClientPartial;
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let observation = observation::Run::start(observation::name(&cli.command));
    let result = execute(cli).await;
    if let Some(observation) = observation {
        observation.finish(if result.is_ok() { 0 } else { 1 });
    }
    result
}

#[locus::trace(with = observation::view())]
async fn execute(cli: Cli) -> Result<()> {
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
    } = cli;
    plumb::identity::ready().map_err(anyhow::Error::msg)?;
    let client = config::client(ClientPartial {
        base_url: base_url.map(Some),
        api_key: api_key.map(Some),
        auth_token_url: auth_token_url.map(Some),
        auth_client_id: auth_client_id.map(Some),
        auth_username: auth_username.map(Some),
        auth_password: auth_password.map(Some),
        token_cache: None,
    })?;
    let defaults = ClientDefaults { strand, soul };
    let bearer = resolve_edge_bearer(Credentials {
        endpoint: client.auth_token_url.as_deref(),
        identity: client.auth_client_id.as_deref(),
        username: client.auth_username.as_deref(),
        password: client.auth_password.as_deref(),
        key: client.api_key.as_deref(),
        cache: client.token_cache.as_deref(),
    })
    .await?;
    let base = client.base_url.unwrap_or_else(|| BASE.to_owned());
    client::run(&base, bearer.as_deref(), &defaults, command).await
}
