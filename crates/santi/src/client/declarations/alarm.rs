use super::super::{ClientDefaults, Http};
use crate::cli::alarm::Alarm;
use anyhow::Result;

pub(in crate::client) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: Alarm,
) -> Result<()> {
    match command {
        Alarm::Set { name, cron, text, generation } => {
            http.post(&format!("{base}/api/v1/alarms"), Some(serde_json::json!({"soul":defaults.require()?,"strand":defaults.resolve_strand(None)?,"name":name,"cron":cron,"text":text,"generation":generation}))).await
        }
        Alarm::List => http.owned(&format!("{base}/api/v1/alarms"), defaults.require()?).await,
        Alarm::Get { id } => http.owned(&format!("{base}/api/v1/alarms/{id}"), defaults.require()?).await,
        Alarm::End { id } => {
            http.delete(&format!("{base}/api/v1/alarms/{id}?soul={}", super::super::urlencoding_encode(defaults.require()?))).await
        }
        Alarm::Fire { id, generation } => {
            let minute = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() / 60;
            http.post(&format!("{base}/api/v1/alarms/{id}/fire"), Some(serde_json::json!({"generation":generation,"minute":minute}))).await
        }
    }
}
