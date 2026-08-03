use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{compact, message, strand};

#[tokio::test]
async fn capsule() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(FakeProvider::default()),
    )
    .await
    .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "compact this exchange".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    let detail = service
        .strand(&strand.id)
        .await
        .expect("read strand")
        .expect("strand");
    let first = detail.messages.first().expect("first message");
    let last = detail.messages.last().expect("last message");

    let report = service
        .exec(
            &strand.id,
            compact::Exec {
                first: Some(first.message.id.clone()),
                last: Some(last.message.id.clone()),
                from: None,
                to: None,
                summary: "one settled exchange".to_string(),
                capsule: Some(compact::Capsule {
                    source: "test".to_string(),
                    reason: "preserve the compact contract".to_string(),
                    risk: "summary omits exact wording".to_string(),
                    queryability: Some("query the original entries".to_string()),
                }),
                dry: false,
            },
        )
        .await
        .expect("create compact capsule");

    assert!(!report.dry);
    assert!(report.before.is_some());
    assert!(report.after.is_some());
    let runtime = service
        .snapshot(&strand.id)
        .await
        .expect("read runtime")
        .expect("runtime");
    assert_eq!(runtime.compacts.len(), 1);
    let compact = &runtime.compacts[0];
    assert_eq!(compact.id, report.compact);
    let metadata = compact.metadata.as_ref().expect("capsule metadata");
    assert_eq!(metadata["schema"], "santi.compact_capsule.v1");
    assert_eq!(metadata["compact"], report.compact);
    assert_eq!(metadata["range"]["start_seq"], first.relation.seq);
    assert_eq!(metadata["range"]["end_seq"], last.relation.seq);
    assert!(metadata.get("after").is_none());
    assert!(metadata.get("ratio").is_none());
    assert_eq!(metadata["forecast"]["authoritative"], false);
    assert_eq!(metadata["forecast"]["basis"], "precommit_preview");
    assert!(metadata["forecast"]["after"]["total"].is_number());
    assert!(metadata["forecast"]["ratio"].is_number());
}
