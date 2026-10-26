mod wake;
pub use wake::{WakeLease, WakeOfferDraft};
mod bootstrap;
pub(super) mod entry;
mod environ;
pub(super) mod read;
mod trace;
pub(super) mod transition;
pub(super) mod write;

use keel::adapt::db::Sqlite;
use std::path::Path;

pub use bootstrap::{Bootstrap, Status};
pub use environ::EnvironDraft;
pub use trace::TraceDraft;

pub(super) async fn wire(path: &Path) -> Result<Sqlite, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if let Some(quarantine) = transition::prepare(path).await? {
        eprintln!(
            "santi-estate: quarantined legacy database at {}",
            quarantine.display()
        );
    }
    let mut wire = Sqlite::file(path).await.map_err(read::error)?;
    fresh(&mut wire, path).await?;
    Ok(wire)
}

const RETIRED: [&str; 5] = [
    "Environ",
    "ErrorIncident",
    "ResolvedIncident",
    "ErrorTransition",
    "ErrorAcknowledgement",
];

async fn fresh(wire: &mut Sqlite, path: &Path) -> Result<(), String> {
    let Ok(stored) = keel::estate::read(wire).await else {
        return Ok(());
    };
    match RETIRED
        .into_iter()
        .find(|unit| stored.nodes().values().any(|spec| spec.name() == *unit))
    {
        Some(unit) => Err(format!(
            "estate at {} was written by an earlier santi model (it holds {unit}); \
             back it up and start an empty estate",
            path.display()
        )),
        None => Ok(()),
    }
}
