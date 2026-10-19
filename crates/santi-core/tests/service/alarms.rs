use super::support::*;
use santi_core::{
    alarm,
    service::{self, Service, alarms::Endpoint},
};
use std::sync::atomic::{AtomicBool, Ordering};

struct Flaky(AtomicBool);

impl Endpoint for Flaky {
    fn validate(&self, _: &alarm::Draft) -> Result<(), String> {
        Ok(())
    }
    fn apply(&self, _: &[alarm::Alarm]) -> Result<alarm::Projection, String> {
        if self.0.load(Ordering::SeqCst) {
            return Err("endpoint temporarily unavailable".into());
        }
        Ok(alarm::Projection {
            endpoint: "test".into(),
            timezone: "UTC".into(),
            applied: true,
            observed: None,
            error: None,
        })
    }
}

#[tokio::test]
async fn recovery() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let endpoint = Arc::new(Flaky(AtomicBool::new(true)));
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(FakeProvider::default()),
    )
    .await
    .unwrap()
    .alarmed(endpoint.clone())
    .await;
    let brain = service.weave().await.unwrap().strand;
    let input = alarm::Draft {
        soul: brain.soul.clone(),
        strand: brain.id.clone(),
        name: "follow".into(),
        text: "check".into(),
        cron: "*/15 * * * *".into(),
        generation: None,
    };
    let first = service.set_alarm(input.clone()).await.unwrap();
    assert!(!first.projection.applied);
    assert_eq!(service.alarms(&brain.soul).await.unwrap().alarms.len(), 1);
    endpoint.0.store(false, Ordering::SeqCst);
    let watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };
    let limit = tokio::time::Instant::now() + Duration::from_secs(8);
    let fixed = loop {
        let view = service.alarms(&brain.soul).await.unwrap();
        if view.projection.applied {
            break view;
        }
        assert!(
            tokio::time::Instant::now() < limit,
            "adaptor did not recover without a caller retry"
        );
        sleep(Duration::from_millis(25)).await;
    };
    assert_eq!(first.alarms[0].id, fixed.alarms[0].id);
    assert!(fixed.projection.observed.is_some());
    service.close();
    watcher.await.unwrap();
    let mut invalid = input;
    invalid.text = " ".into();
    assert!(service.set_alarm(invalid).await.is_err());
    service
        .end_alarm(&fixed.alarms[0].id, &brain.soul)
        .await
        .unwrap();
    assert!(service.alarms(&brain.soul).await.unwrap().alarms.is_empty());
}
