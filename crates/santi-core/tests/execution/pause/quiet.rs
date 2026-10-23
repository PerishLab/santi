use super::super::super::support::*;
use super::reply::{Spoken, spoken};
use santi_core::service;

async fn resumed(service: &service::Service, strand: &str) -> santi_core::stream::Snapshot {
    for _ in 0..300 {
        let runtime = service.snapshot(strand).await.unwrap().unwrap();
        if runtime
            .messages
            .iter()
            .any(|message| message.text == "resumed")
        {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("strand did not resume");
}

#[tokio::test]
async fn quiet() {
    for replies in [0, 2] {
        let Spoken {
            _temp,
            service,
            strand,
            ..
        } = spoken(replies, None).await;
        let runtime = resumed(&service, &strand).await;
        assert_eq!(runtime.compacts.len(), 1);
        assert!(
            runtime
                .messages
                .iter()
                .all(|message| !message.text.contains("kind: compact_reminder"))
        );
    }
}
