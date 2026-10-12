use santi_core::{
    job,
    service::{JobLaunch, JobTerminal},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Spec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<santi_estate::Environment>,
    pub schema: String,
    pub id: String,
    pub origin: job::Origin,
    #[serde(alias = "generation")]
    pub stamp: String,
    #[serde(alias = "unit")]
    pub sidecar: String,
    pub description: String,
    pub command: String,
    pub cwd: String,
    #[serde(rename = "output_limit_bytes")]
    pub output: u64,
}

#[derive(PartialEq, Eq)]
struct Identity<'a> {
    environment: Option<&'a santi_estate::Environment>,
    id: &'a str,
    origin: &'a job::Origin,
    stamp: &'a str,
    sidecar: &'a str,
    description: &'a str,
    command: &'a str,
    cwd: &'a str,
    output: u64,
}

impl Spec {
    pub(super) fn from(launch: &JobLaunch) -> Self {
        Self {
            environment: launch.environment.clone(),
            schema: "santi.job.execution.v2".to_string(),
            id: launch.job.id.clone(),
            origin: launch.job.origin.clone(),
            stamp: launch.stamp.clone(),
            sidecar: launch.sidecar.clone(),
            description: launch.job.description.clone(),
            command: launch.job.command.clone(),
            cwd: launch.cwd.clone(),
            output: launch.job.output_limit_bytes,
        }
    }

    pub(super) fn legacy(&self) -> bool {
        self.schema == "santi.job.execution.v1"
    }

    pub(super) fn matches(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }

    fn identity(&self) -> Identity<'_> {
        Identity {
            environment: self.environment.as_ref(),
            id: &self.id,
            origin: &self.origin,
            stamp: &self.stamp,
            sidecar: &self.sidecar,
            description: &self.description,
            command: &self.command,
            cwd: &self.cwd,
            output: self.output,
        }
    }
}

pub(super) type Record = JobTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Claimed,
    Running,
    Terminal,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Snapshot {
    pub schema: String,
    pub phase: Phase,
}

impl Snapshot {
    pub(super) fn new(phase: Phase) -> Self {
        Self {
            schema: "santi.job.state.v1".to_string(),
            phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Spec;

    #[test]
    fn legacy() {
        let value = serde_json::json!({
            "schema": "santi.job.execution.v1", "id": "job_test",
            "origin": { "soul": "soul_test", "strand": "strand_test", "turn": "turn_test",
                "call": "call_test", "effect": "effect_test" },
            "generation": "generation_test", "unit": "unit_test",
            "description": "legacy", "command": "true", "cwd": "/tmp", "output_limit_bytes": 1024
        });
        let legacy: Spec = serde_json::from_value(value.clone()).expect("legacy spec");
        assert!(legacy.legacy());
        assert!(legacy.environment.is_none());
        let mut frozen = value;
        frozen["environment"] = serde_json::json!({"PRIVATE_PROBE": "private-probe-value"});
        let snapshot: Spec = serde_json::from_value(frozen.clone()).expect("snapshot");
        assert!(!legacy.matches(&snapshot));
        assert!(!format!("{snapshot:?}").contains("private-probe-value"));
        assert!(snapshot.matches(&serde_json::from_value(frozen.clone()).expect("same snapshot")));
        frozen["environment"]["PRIVATE_PROBE"] = "changed".into();
        assert!(!snapshot.matches(&serde_json::from_value(frozen).expect("different snapshot")));
    }

    #[test]
    fn privacy() {
        let private = "fictional-sensitive-spec-value";
        let value = serde_json::json!({
            "schema": "santi.job.execution.v2", "id": "job_test",
            "origin": { "soul": "soul_test", "strand": "strand_test", "turn": "turn_test",
                "call": "call_test", "effect": "effect_test" },
            "generation": "generation_test", "unit": "unit_test",
            "description": "privacy", "command": "true", "cwd": "/tmp", "output_limit_bytes": 1024,
            "environment": private
        });
        let bytes = serde_json::to_vec_pretty(&value).expect("encode malformed spec");
        let raw = serde_json::from_slice::<Spec>(&bytes).expect_err("invalid environment type");
        assert!(raw.to_string().contains(private));
        let temporary = tempfile::tempdir().expect("private job directory");
        let directory = super::super::files::directory(temporary.path());
        directory.prepare().expect("prepare directory");
        std::fs::write(temporary.path().join(super::super::files::SPEC), bytes)
            .expect("write malformed spec");
        let error = directory.spec().expect_err("invalid private spec");
        assert!(!error.contains(private));
        assert!(error.contains("job spec.json (Data)"));
        assert!(error.contains(&format!("line {} column {}", raw.line(), raw.column())));
    }
}
