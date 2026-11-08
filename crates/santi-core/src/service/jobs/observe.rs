use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use santi_estate::JobRecord;

#[derive(PartialEq, Eq)]
struct Status<'a> {
    state: &'a job::State,
    reason: Option<&'a str>,
    exit: Option<i32>,
    complete: bool,
}

impl Service {
    pub(super) async fn refresh(&self, record: JobRecord) -> Result<JobRecord, String> {
        if record.job.acknowledged.is_some() {
            return Ok(record);
        }
        if self
            .handoffs
            .lock()
            .unwrap()
            .contains_key(&record.generation)
        {
            return Ok(record);
        }
        let observation = self.supervisor.observe(&self.launch(&record)?)?;
        let claimed = matches!(
            &observation,
            Observation::Claimed | Observation::Running | Observation::Terminal(_)
        );
        let record = if claimed && record.job.state == job::State::Submitting {
            self.store.accept_job(&record.job.id, &crate::now()).await?
        } else {
            record
        };
        let transition = match observation {
            Observation::Claimed => None,
            Observation::Running => Some((job::State::Running, None, None)),
            Observation::Terminal(terminal) => {
                Some((terminal.state, terminal.reason, terminal.exit))
            }
            Observation::Aborted => Some((
                job::State::Failed,
                Some("submission_aborted".to_string()),
                None,
            )),
            Observation::Missing if record.job.state.terminal() => None,
            Observation::Missing if record.job.state == job::State::Submitting => Some((
                job::State::Unknown,
                Some("submission_unknown".to_string()),
                None,
            )),
            Observation::Missing => Some((
                job::State::Unknown,
                Some("sidecar_evidence_missing".to_string()),
                None,
            )),
        };
        let Some((state, reason, exit)) = transition else {
            return Ok(record);
        };
        let incomplete = state == job::State::Running && record.started_millis.is_none();
        let current = Status {
            state: &record.job.state,
            reason: record.job.reason.as_deref(),
            exit: record.job.exit_code,
            complete: !incomplete,
        };
        let desired = Status {
            state: &state,
            reason: reason.as_deref(),
            exit,
            complete: true,
        };
        if current == desired {
            return Ok(record);
        }
        self.store
            .transition_job(
                &record.job.id,
                santi_estate::TransitionDraft {
                    base: Some(&record.job.state),
                    reason: reason.as_deref(),
                    exit_code: exit,
                    started_millis: incomplete.then(epoch).transpose()?,
                    ..santi_estate::TransitionDraft::new(state, &crate::now())
                },
            )
            .await
    }
}

pub(super) fn epoch() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis(),
    )
    .map_err(|_| "current time is out of range".to_string())
}
