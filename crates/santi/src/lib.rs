pub mod inference;

#[cfg(feature = "client")]
pub mod auth;
#[cfg(feature = "client")]
pub mod cli;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub mod config;
#[cfg(feature = "client")]
#[path = "cli/observation.rs"]
mod observation;
#[cfg(feature = "client")]
mod text;
#[cfg(feature = "client")]
pub mod watch;

#[cfg(feature = "client")]
use anyhow::Result;
#[cfg(feature = "client")]
use auth::{Credentials, resolve};
#[cfg(feature = "client")]
use cli::{BASE, Cli, Command, Defaults};
#[cfg(feature = "client")]
use config::ClientPartial;
#[cfg(feature = "client")]
pub async fn run() -> Result<()> {
    let (cli, explicit) = crate::cli::parse()?;
    let observation = observation::start(&cli.command);
    let result = execute(cli, explicit).await;
    if let Some(observation) = observation {
        observation.finish(if result.is_ok() { 0 } else { 1 });
    }
    result
}

#[cfg(feature = "client")]
#[locus::trace(with = plumb::observation::view())]
async fn execute(cli: Cli, explicit: bool) -> Result<()> {
    let Cli {
        endpoint,
        key,
        token,
        client: identity,
        username,
        password,
        strand,
        soul,
        command,
    } = cli;
    plumb::identity::ready().map_err(anyhow::Error::msg)?;
    if let Command::Skill(deed) = command {
        return cli::skill::run(deed);
    }
    let stated = cli::declaration(&command)?;
    let client = config::client(ClientPartial {
        operator: None,
        endpoint: endpoint.map(Some),
        key: key.map(Some),
        token: token.map(Some),
        client: identity.map(Some),
        username: username.map(Some),
        password: password.map(Some),
        cache: None,
        inference: stated.partial(),
    })?;
    let defaults = Defaults::new(strand, soul, &client);
    let bearer = resolve(Credentials {
        endpoint: client.token.as_deref(),
        identity: client.client.as_deref(),
        username: client.username.as_deref(),
        password: client.password.as_deref(),
        key: client.key.as_deref(),
        cache: client.cache.as_deref(),
    })
    .await?;
    let base = client.endpoint.unwrap_or_else(|| BASE.to_owned());
    client::run(
        client::Run {
            base: &base,
            bearer: bearer.as_deref(),
            defaults: &defaults,
            explicit,
        },
        command,
    )
    .await
}
