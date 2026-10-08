use serde::Serialize;

use crate::config::context::Origin;
use santi_api::ops::DoctorReport;

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
            actions.push("Fix the selected provider configuration or supply its referenced variables to this process; a service's environment is not inherited by this command.");
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
