use std::sync::{Arc, Mutex};

use axum::http::HeaderMap;
use santi_api::{CreateJobRequest, Jobs};
use santi_core::{
    job,
    service::{self, JobLaunch, JobObservation, JobSupervisor, Service},
};

use super::*;

mod fixture;
mod race;

struct FakeSupervisor {
    launches: Mutex<usize>,
}

impl JobSupervisor for FakeSupervisor {
    fn detach(&self, _: &JobLaunch) -> Result<(), String> {
        *self.launches.lock().unwrap() += 1;
        Ok(())
    }

    fn observe(&self, _: &JobLaunch) -> Result<JobObservation, String> {
        Ok(JobObservation::Claimed)
    }

    fn stop(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }

    fn acknowledge(&self, _: &JobLaunch) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn accepts() {
    for cwd in [
        None,
        Some("soul://"),
        Some("soul://nested"),
        Some("strand://"),
        Some("strand://nested"),
    ] {
        exercise(cwd, false).await;
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "requires a running user systemd manager; run explicitly for native execution evidence"]
async fn boots() {
    for cwd in [
        None,
        Some("soul://"),
        Some("soul://nested"),
        Some("strand://"),
        Some("strand://nested"),
    ] {
        exercise(cwd, true).await;
    }
}

async fn exercise(cwd: Option<&str>, native: bool) {
    let temp = tempfile::tempdir().expect("temp dir");
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let endpoint: Arc<dyn JobSupervisor> = if native {
        Arc::new(santi_api::jobs::Native::new(
            std::env::var("SANTI_NATIVE_TEST_EXECUTABLE")
                .expect("explicit built santi-api executable"),
        ))
    } else {
        supervisor.clone()
    };
    let (service, strand, store) = fixture::prepare(&temp, endpoint).await;
    let capability = fixture::CAPABILITY;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-santi-job-capability",
        capability.parse().expect("capability header"),
    );
    let request = || CreateJobRequest {
        description: "http boundary probe".to_string(),
        command: "pwd".to_string(),
        cwd: cwd.map(str::to_string),
        timeout_seconds: Some(30),
        output_limit_bytes: Some(4096),
        remind_every_seconds: Some(5),
    };

    for cwd in [
        "/private/secret-path",
        "@secret",
        "https://secret",
        "soul://../secret",
        "strand:///secret",
    ] {
        let mut invalid = request();
        invalid.cwd = Some(cwd.to_string());
        let error = Jobs(service.clone())
            .create(headers.clone(), invalid)
            .await
            .expect_err("invalid cwd refused");
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(
            error
                .message()
                .starts_with("job cwd must be a workspace URI:")
        );
        assert!(!error.message().contains("secret"));
        assert!(store.jobs(&strand.soul).await.expect("jobs").is_empty());
        assert_eq!(*supervisor.launches.lock().unwrap(), 0);
        assert!(!temp.path().join("runtime/jobs").exists());
    }

    let (status, Json(first)) = Jobs(service.clone())
        .create(headers.clone(), request())
        .await
        .unwrap_or_else(|error| panic!("create failed: {}", error.message()));
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(first.job.state, job::State::Accepted);
    assert_eq!(first.job.origin.soul, strand.soul);
    assert_eq!(first.job.remind, Some(5));

    let (_, Json(retried)) = Jobs(service.clone())
        .create(headers, request())
        .await
        .unwrap_or_else(|error| panic!("retry failed: {}", error.message()));
    assert_eq!(retried.job.id, first.job.id);
    if !native {
        assert_eq!(*supervisor.launches.lock().unwrap(), 1);
    } else {
        let completed = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            terminal(&service, &strand.soul, &first.job.id),
        )
        .await
        .expect("native job finishes");
        assert_eq!(completed.state, job::State::Succeeded);
        let expected = match cwd {
            None => temp.path().join("execution"),
            Some(uri) if uri.starts_with("soul://") => temp
                .path()
                .join("runtime/souls")
                .join(&strand.soul)
                .join("memory")
                .join(uri.trim_start_matches("soul://")),
            Some(uri) => temp
                .path()
                .join("runtime/strands")
                .join(&strand.id)
                .join("memory")
                .join(uri.trim_start_matches("strand://")),
        };
        let log = service
            .logs(service::JobRead {
                soul: &strand.soul,
                id: &first.job.id,
                stream: job::Stream::Stdout,
                cursor: "0",
                limit: 4096,
            })
            .await
            .expect("logs")
            .expect("owned logs");
        assert_eq!(
            std::fs::canonicalize(log.data.trim()).expect("reported cwd"),
            std::fs::canonicalize(expected).expect("expected cwd")
        );
        service.ack(&strand.soul, &first.job.id).await.expect("ack");
    }

    let mut owner = HeaderMap::new();
    owner.insert("x-santi-soul-id", strand.soul.parse().expect("soul header"));
    let Json(queried) = Jobs(service)
        .get(owner, first.job.id.clone())
        .await
        .unwrap_or_else(|error| panic!("get failed: {}", error.message()));
    assert_eq!(queried.id, first.job.id);
}

async fn terminal(service: &Service, soul: &str, id: &str) -> job::Job {
    loop {
        let held = service.job(soul, id).await.expect("observe").expect("job");
        if held.state.terminal() {
            return held;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}
