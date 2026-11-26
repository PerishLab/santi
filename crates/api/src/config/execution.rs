use radicula_config::Cascade;
use serde::Serialize;

#[derive(Debug, Serialize, Cascade)]
#[cascade(section)]
pub struct Execution {
    pub profile: String,
    pub timeout: u64,
    pub rounds: usize,
    pub calls: usize,
    pub output: usize,
    pub shell: usize,
    pub lines: usize,
    #[cascade(name = "feedback_after_calls")]
    #[serde(rename = "feedback_after_calls")]
    pub after: Option<usize>,
    #[cascade(name = "feedback_command")]
    #[serde(rename = "feedback_command")]
    pub command: Option<String>,
    #[cascade(name = "feedback_cwd")]
    #[serde(rename = "feedback_cwd")]
    pub cwd: Option<String>,
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            profile: "runtime_v1".to_string(),
            timeout: santi_core::budget::timeout(),
            rounds: 64,
            calls: 256,
            output: 4 * 1024 * 1024,
            shell: 8192,
            lines: 256,
            after: None,
            command: None,
            cwd: None,
        }
    }
}

impl Execution {
    pub fn budget(&self) -> Result<santi_core::budget::Execution, String> {
        let value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let budget = serde_json::from_value::<santi_core::budget::Execution>(value)
            .map_err(|error| error.to_string())?;
        budget.validate()?;
        Ok(budget)
    }
}
