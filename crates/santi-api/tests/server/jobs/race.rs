use super::*;
use santi_estate::JobDraft;
use sha2::{Digest, Sha256};
use std::{sync::mpsc, time::Duration};

struct Paused {
    captured: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
}

impl JobSupervisor for Paused {
    fn detach(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }

    fn observe(&self, _: &JobLaunch) -> Result<JobObservation, String> {
        self.captured.send(()).expect("capture missing observation");
        self.resume
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .expect("release observation");
        Ok(JobObservation::Missing)
    }

    fn stop(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }

    fn acknowledge(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (captured, observed) = mpsc::sync_channel(1);
    let (resume, gate) = mpsc::sync_channel(1);
    let supervisor = Arc::new(Paused {
        captured,
        resume: Mutex::new(gate),
    });
    let (service, strand, store) = fixture::prepare(&temp, supervisor).await;
    prepare(&store).await;
    let observer = tokio::spawn(async move { service.job(&strand.soul, "job_race").await });
    observed
        .recv_timeout(Duration::from_secs(5))
        .expect("observer captured pre-handoff absence");
    store
        .accept_job("job_race", &santi_core::now())
        .await
        .expect("handoff accepted before observer returns");
    resume.send(()).expect("release stale observation");
    let job = observer
        .await
        .expect("observer task")
        .expect("observe")
        .expect("job");
    assert_eq!(job.state, job::State::Accepted);
    assert!(job.finished.is_none());
}

async fn prepare(store: &santi_core::Store) {
    let digest = format!("{:x}", Sha256::digest(fixture::CAPABILITY.as_bytes()));
    store
        .prepare_job(
            &digest,
            JobDraft {
                tag: "job_race",
                description: "stale observer",
                command: "true",
                environment: None,
                cwd: None,
                timeout_seconds: 30,
                output_limit_bytes: 4096,
                remind_every_seconds: None,
                request_sha256: "request_digest",
                generation: "stamp_race",
                supervisor_ref: "sidecar_race",
                created: &santi_core::now(),
            },
            0,
        )
        .await
        .expect("prepare job");
}

struct Evidence(JobObservation);

impl JobSupervisor for Evidence {
    fn detach(&self, _: &JobLaunch) -> Result<(), String> {
        Err("submission probe refusal".to_string())
    }

    fn observe(&self, _: &JobLaunch) -> Result<JobObservation, String> {
        Ok(self.0.clone())
    }

    fn stop(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }

    fn acknowledge(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn evidence() {
    for (observation, expected) in [
        (JobObservation::Missing, job::State::Unknown),
        (JobObservation::Aborted, job::State::Failed),
        (JobObservation::Claimed, job::State::Accepted),
        (
            JobObservation::Terminal(service::JobTerminal {
                state: job::State::Succeeded,
                reason: None,
                exit: Some(0),
            }),
            job::State::Succeeded,
        ),
    ] {
        let temp = tempfile::tempdir().expect("temp dir");
        let (service, strand, store) =
            fixture::prepare(&temp, Arc::new(Evidence(observation))).await;
        prepare(&store).await;
        let job = service
            .job(&strand.soul, "job_race")
            .await
            .expect("observe")
            .expect("job");
        assert_eq!(job.state, expected);
        assert_eq!(
            job.accepted.is_some(),
            matches!(expected, job::State::Accepted | job::State::Succeeded)
        );
    }
}

#[tokio::test]
async fn releases() {
    let temp = tempfile::tempdir().expect("temp dir");
    let (service, strand, _) =
        fixture::prepare(&temp, Arc::new(Evidence(JobObservation::Missing))).await;
    assert!(
        service
            .spawn(
                fixture::CAPABILITY,
                service::JobDraft {
                    description: "refused handoff".to_string(),
                    command: "true".to_string(),
                    cwd: None,
                    timeout: Some(30),
                    output: Some(4096),
                    remind: None,
                }
            )
            .await
            .is_err()
    );
    let jobs = service
        .jobs(&strand.soul)
        .await
        .expect("query after refusal");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].state, job::State::Unknown);
    assert!(jobs[0].accepted.is_none());
}
