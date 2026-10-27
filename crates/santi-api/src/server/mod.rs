mod effects;
mod error;
mod errors;
mod ingress;
mod jobs;
mod openapi;
mod routes;
mod shutdown;
mod sse;

use santi_core::service::{self, Service};
use std::{fs, net::SocketAddr};

use crate::provider;

pub use effects::{ResolveEffectRequest, effect, settle};
pub use error::ApiError;
pub use errors::{ErrorQueryParams, errors, incident, stranded};
pub use jobs::{CreateJobRequest, Jobs};
pub use routes::{Environs, drive, end_strand, health, send};

pub fn export_openapi_json() -> Result<String, String> {
    serde_json::to_string_pretty(&openapi::document()).map_err(|error| error.to_string())
}

pub async fn serve() -> Result<(), String> {
    let held = crate::runtime::held();
    let provider = provider::build(held.resolved()?);
    let bind = held.bind.clone();
    let paths = held.paths.clone();
    if let Some(parent) = paths.database.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&paths.runtime).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paths.execution).map_err(|error| error.to_string())?;
    let supervisor = std::sync::Arc::new(crate::jobs::Native::current()?);
    let service = Service::supervised(
        service::Config {
            database: paths.database.display().to_string(),
            runtime: paths.runtime.display().to_string(),
            execution: paths.execution.display().to_string(),
            bind: Some(bind.clone()),
            constitution: held
                .constitution
                .as_ref()
                .map(|path| path.display().to_string()),
            environment: held.environment.clone(),
        },
        provider,
        supervisor,
    )
    .await?
    .identified(held.souls.clone())
    .await?
    .bounded(held.execution.clone())?
    .memory(held.memory)?
    .catalogued(held.materials.clone())?
    .authorized(held.capability.clone())
    .retain(held.retention)?
    .alarmed(std::sync::Arc::new(crate::adaptor::cron::Cron::current()))
    .await;
    let address: SocketAddr = bind
        .parse()
        .map_err(|_| "listen host/port did not form a valid socket address".to_string())?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|error| error.to_string())?;
    println!("santi-api listening on http://{address}");
    shutdown::serve(service, listener, held.grace, wait_for_shutdown_signal()).await
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(term) => term,
            Err(error) => {
                eprintln!("santi-api: cannot install SIGTERM handler: {error}");
                return;
            }
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
