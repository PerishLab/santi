use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::HeaderMap;
use santi_api::{CreateJobRequest, create_job_handler, get_job_handler};
use santi_core::{
    job,
    service::{self, JobLaunch, JobObservation, JobSupervisor, Service},
};
use santi_estate::{CallDraft, CapabilityDraft, EffectDraft, TurnDraft};
use sha2::{Digest, Sha256};

use super::*;

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
    let database = temp.path().join("santi.sqlite");
    super::support::bootstrap(&database).await;
    let supervisor = Arc::new(FakeSupervisor {
        launches: Mutex::new(0),
    });
    let service = Service::supervised(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
        if native {
            Arc::new(santi_api::jobs::Native::new(
                std::env::var("SANTI_NATIVE_TEST_EXECUTABLE")
                    .expect("explicit built santi-api executable"),
            )) as Arc<dyn JobSupervisor>
        } else {
            supervisor.clone()
        },
    )
    .await
    .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    let capability = "jobcap_http_probe";
    let digest = format!("{:x}", Sha256::digest(capability.as_bytes()));
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
        + 60_000;
    let store = santi_core::Store::open(&database)
        .await
        .expect("open estate");
    store
        .create_turn(TurnDraft {
            tag: "turn_http",
            strand: &strand.id,
            trigger: santi_core::turn::Trigger::System,
            source: None,
            from: 0,
            created: &santi_core::now(),
        })
        .await
        .expect("turn");
    store
        .create_call(CallDraft {
            tag: "call_http",
            turn: "turn_http",
            tool: "shell",
            arguments: &serde_json::json!({"command": "printf ok"}),
            created: &santi_core::now(),
        })
        .await
        .expect("call");
    store
        .prepare_effect(EffectDraft {
            tag: "effect_http",
            turn: "turn_http",
            call: Some("call_http"),
            kind: "shell",
            metadata: None,
            created: &santi_core::now(),
        })
        .await
        .expect("effect");
    store
        .create_capability(CapabilityDraft {
            digest: &digest,
            expires: expiry,
            soul: &strand.soul,
            strand: &strand.id,
            turn: "turn_http",
            call: "call_http",
            effect: "effect_http",
            created: &santi_core::now(),
        })
        .await
        .expect("seed capability");
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
        let error = create_job_handler(State(service.clone()), headers.clone(), Json(invalid))
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

    let (status, Json(first)) =
        create_job_handler(State(service.clone()), headers.clone(), Json(request()))
            .await
            .unwrap_or_else(|error| panic!("create failed: {}", error.message()));
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(first.job.state, job::State::Accepted);
    assert_eq!(first.job.origin.soul, strand.soul);
    assert_eq!(first.job.remind, Some(5));

    let (_, Json(retried)) = create_job_handler(State(service.clone()), headers, Json(request()))
        .await
        .unwrap_or_else(|error| panic!("retry failed: {}", error.message()));
    assert_eq!(retried.job.id, first.job.id);
    if !native {
        assert_eq!(*supervisor.launches.lock().unwrap(), 1);
    } else {
        let completed = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                let held = service
                    .job(&strand.soul, &first.job.id)
                    .await
                    .expect("observe")
                    .expect("job");
                if held.state.terminal() {
                    break held;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        })
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
    let Json(queried) = get_job_handler(State(service), owner, Path(first.job.id.clone()))
        .await
        .unwrap_or_else(|error| panic!("get failed: {}", error.message()));
    assert_eq!(queried.id, first.job.id);
}
