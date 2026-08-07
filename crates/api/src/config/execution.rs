use plumb::config::Cascade;
use serde::Serialize;

#[derive(Debug, Serialize, Cascade)]
#[cascade(section)]
pub struct Execution {
    pub profile: String,
    pub rounds: usize,
    pub calls: usize,
    pub output: usize,
    pub shell: usize,
    pub feedback_after_calls: Option<usize>,
    pub feedback_command: Option<String>,
    pub feedback_cwd: Option<String>,
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            profile: "runtime_v1".to_string(),
            rounds: 16,
            calls: 256,
            output: 4 * 1024 * 1024,
            shell: 64 * 1024,
            feedback_after_calls: None,
            feedback_command: None,
            feedback_cwd: None,
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
