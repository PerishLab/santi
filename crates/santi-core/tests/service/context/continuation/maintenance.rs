use super::helpers::{explicit, inflated, maintained, offers};
use super::*;

#[tokio::test]
async fn successor() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;

    let setup = Arc::new(Script::new(vec![
        inflated(tools(), 196_608),
        finish("answer"),
    ]));
    let service = Service::open(config(&temp), setup.clone()).await.unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    let first = messages.first().unwrap().message.id.clone();
    let last = messages.last().unwrap().message.id.clone();
    let before = service
        .audit(&strand.id)
        .await
        .unwrap()
        .unwrap()
        .request
        .estimate
        .total;
    let bytes = (before * 4 / 5).max(1);
    drop(service);

    let mut provider = Script::new(vec![
        explicit("call_compact", "retain the whole history", &first, &last),
        finish("continued"),
    ]);
    provider.limit = Some(bytes as usize);
    let provider = Arc::new(provider);
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let pressured = service
        .audit(&strand.id)
        .await
        .unwrap()
        .unwrap()
        .pressure
        .expect("the fixture must open over the ordinary active allowance");
    assert!(
        pressured.active > pressured.ceiling,
        "the fixture must start under maintenance pressure"
    );
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "continue".into(),
                }],
            },
        )
        .await
        .unwrap();

    let maintenance = maintained(&service, &strand.id).await;
    let snapshot = service.snapshot(&strand.id).await.unwrap().unwrap();
    let held = snapshot
        .turns
        .iter()
        .find(|turn| turn.id == maintenance)
        .expect("the compact call belongs to a recorded Turn");
    assert_eq!(
        held.trigger,
        santi_core::turn::Trigger::System,
        "preemptive maintenance opens a system Turn"
    );
    assert_ne!(held.status, santi_core::turn::Status::Failed);
    Probe::new(&service)
        .completed_turn(&strand.id, &maintenance)
        .await;

    let successor = accepted_turn(&service, &posted).await;
    assert_ne!(
        successor.id, maintenance,
        "the posted receipt drives a successor Turn"
    );
    Probe::new(&service)
        .completed_turn(&strand.id, &successor.id)
        .await;

    {
        let requests = provider.requests.lock().unwrap();
        assert!(requests.len() >= 2, "maintenance and its successor request");
        assert!(
            offers(&requests[0]),
            "the maintenance round must offer compact"
        );
        let next = requests.last().unwrap();
        assert!(!offers(next), "the successor must be an ordinary round");
        assert!(
            next.tools.as_ref().is_some_and(|tools| !tools.is_empty()),
            "the successor request must carry ordinary tools"
        );
        provider
            .chat
            .validate(next)
            .expect("the successor history must validate");
        assert!(
            next.input.iter().any(
                |item| matches!(item, Item::Message { content, .. } if content.contains("retain the whole history"))
            ),
            "the successor must reload the committed compact summary"
        );
    }

    let settled = service.audit(&strand.id).await.unwrap().unwrap();
    let after = settled.request.estimate.total;
    let balanced = settled
        .pressure
        .expect("the committed compact must restore an active allowance");
    assert!(
        after < before,
        "the compact must remove the synthetic history"
    );
    assert!(
        balanced.active <= balanced.ceiling,
        "the successor must return under the ordinary active allowance"
    );

    let snapshot = service.snapshot(&strand.id).await.unwrap().unwrap();
    assert_eq!(
        snapshot.compacts.len(),
        1,
        "exactly one Compact is committed"
    );
    assert_eq!(
        snapshot.compacts[0].last, last,
        "the explicit range owns the whole history"
    );
    assert_eq!(
        snapshot
            .calls
            .iter()
            .filter(|call| call.tool == "compact")
            .count(),
        1
    );
    assert!(
        snapshot
            .turns
            .iter()
            .all(|turn| turn.status != santi_core::turn::Status::Failed)
    );
}
