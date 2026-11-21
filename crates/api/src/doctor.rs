use serde::Serialize;

use crate::config::context::Origin;
use santi_api::ops::DoctorReport;

pub async fn inspect(
    storage_only: bool,
    jobs: bool,
) -> Result<(DoctorReport, Option<santi_api::jobs::readiness::Report>), String> {
    let storage = async {
        if storage_only {
            santi_api::runtime::held().paths.doctor().await
        } else {
            santi_api::ops::doctor().await
        }
    };
    tokio::try_join!(storage, probes(jobs))
}

async fn probes(jobs: bool) -> Result<Option<santi_api::jobs::readiness::Report>, String> {
    if !jobs {
        return Ok(None);
    }
    let held = santi_api::runtime::held();
    Ok(Some(
        santi_api::jobs::readiness::inspect(&held.environment).await,
    ))
}

#[derive(Serialize)]
pub struct Report {
    config: Origin,
    target: &'static str,
    state: &'static str,
    actions: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jobs: Option<santi_api::jobs::readiness::Report>,
    #[serde(flatten)]
    report: DoctorReport,
}

impl Report {
    pub fn new(
        config: Origin,
        report: DoctorReport,
        jobs: Option<santi_api::jobs::readiness::Report>,
    ) -> Self {
        let mut actions = Vec::new();
        let state = if !report.database_exists {
            actions.push("Run santi-api bootstrap with this same --config to initialize the estate and sudo custody.");
            "unbootstrapped"
        } else if !report.estate_bound {
            actions.push("Inspect estate_error; restore the estate and its sudo custody together from a compatible backup.");
            "unavailable"
        } else if !report.estate_ready {
            actions.push("Run santi-api serve with this same --config; serving initializes the missing default soul.");
            "genesis_missing"
        } else {
            "ready"
        };
        if report
            .provider
            .as_ref()
            .is_some_and(|provider| !provider.ok)
        {
            actions.push("Check the scenario, its preferred candidate and custody, and supply any referenced variables to this process; a service's environment is not inherited by this command.");
        }
        if report.memory_present && !report.memory_readable {
            actions.push("Restore read access to memory_path for the intended runtime user.");
        }
        Self {
            config,
            target: "local_process",
            state,
            actions,
            jobs,
            report,
        }
    }
}
