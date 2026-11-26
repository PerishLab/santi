use std::path::PathBuf;

use super::{Launch, Service};
use santi_estate::Record;

impl Service {
    pub(super) fn jobhome(&self, record: &Record) -> PathBuf {
        let key = if record.generation.starts_with("stamp_") {
            &record.generation
        } else {
            &record.job.id
        };
        self.runtime().join("jobs").join(key)
    }
}

impl Service {
    pub(super) fn launch(&self, record: &Record) -> Result<Launch, String> {
        let cwd = self.situated(
            &record.job.origin.strand,
            &record.job.origin.soul,
            record.job.cwd.as_deref(),
        )?;
        std::fs::create_dir_all(&cwd).map_err(|error| error.to_string())?;
        let endpoint = self
            .config
            .bind
            .as_ref()
            .map(|bind| format!("http://{bind}"));
        Ok(Launch {
            environment: record.environment.clone(),
            endpoint,
            job: record.job.clone(),
            stamp: record.generation.clone(),
            sidecar: record.supervisor.clone(),
            cwd: cwd.display().to_string(),
            directory: self.jobhome(record).display().to_string(),
        })
    }
}
