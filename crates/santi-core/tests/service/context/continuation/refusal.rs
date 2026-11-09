use super::helpers::{explicit, inflated, offers};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct Gate {
    inner: Arc<Script>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    armed: AtomicBool,
}

#[async_trait]
impl Provider for Gate {
    fn metadata(&self) -> Metadata {
        self.inner.metadata()
    }
    fn prepare(&self, request: Request) -> Result<Request, String> {
        self.inner.prepare(request)
    }
    fn validate(&self, request: &Request) -> Result<(), String> {
        self.inner.validate(request)
    }
    fn estimate(&self, request: &Request) -> Option<santi_core::budget::Estimate> {
        self.inner.estimate(request)
    }
    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let retry = self.inner.requests.lock().unwrap().len() == 1
            && self.armed.swap(false, Ordering::SeqCst);
        if retry {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.stream(request).await
    }
}

#[tokio::test]
async fn retry() {
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
    assert!(messages.len() >= 3, "the tool round must be recorded");
    let first = messages.first().unwrap().message.id.clone();
    let last = messages.last().unwrap().message.id.clone();
    let cut = messages[1].message.id.clone();
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

    let mut inner = Script::new(vec![
        explicit(
            "call_unsafe",
            "cut the tool round",
            &messages[0].message.id,
            &cut,
        ),
        explicit("call_compact", "retain the whole history", &first, &last),
        finish("continued"),
    ]);
    inner.limit = Some(bytes as usize);
    let inner = Arc::new(inner);
    let gate = Arc::new(Gate {
        inner: inner.clone(),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
        armed: AtomicBool::new(true),
    });
    let service = Service::open(config(&temp), gate.clone()).await.unwrap();
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

    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
        .await
        .expect("the rejected round must be followed by a same-turn retry");

    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let snapshot = service.snapshot(&strand.id).await.unwrap().unwrap();
    let rejected = snapshot
        .calls
        .iter()
        .filter(|call| call.tool == "compact")
        .collect::<Vec<_>>();
    assert_eq!(rejected.len(), 1, "only the unsafe call so far");
    let maintenance = rejected[0].turn.clone();
    let reply = snapshot
        .results
        .iter()
        .find(|reply| reply.call == rejected[0].id)
        .expect("the rejected compact reply must be recorded");
    let error = reply
        .error
        .as_deref()
        .expect("the refusal must persist an error reply");
    assert!(
        error.contains("compact.anchor") || error.contains("compact.exchange"),
        "the refusal must name the structural conflict: {error}"
    );
    assert!(
        store.compacts(&strand.id).await.unwrap().is_empty(),
        "a refused compact must not create a Compact"
    );
    let current = snapshot
        .turns
        .iter()
        .find(|held| held.id == maintenance)
        .expect("the maintenance Turn is recorded");
    assert_eq!(
        current.status,
        santi_core::turn::Status::Running,
        "the refusal must not stop the Turn"
    );
    assert_eq!(
        current.trigger,
        santi_core::turn::Trigger::System,
        "preemptive maintenance opens a system Turn"
    );
    assert_eq!(snapshot.turns.len(), 2, "the refusal opens no new Turn");
    assert!(
        snapshot
            .messages
            .iter()
            .all(|entry| !entry.text.contains("kind: execution_pause")),
        "the refusal posts no execution_pause message"
    );
    assert!(
        store
            .inboxes(&strand.id)
            .await
            .unwrap()
            .iter()
            .all(|inbox| inbox
                .source
                .as_ref()
                .is_none_or(|source| source.kind != "execution_pause")),
        "no execution_pause is pending in the inbox"
    );
    assert_eq!(
        inner.requests.lock().unwrap().len(),
        1,
        "the rejected round is the first request of this maintenance Turn"
    );

    gate.release.notify_one();
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
        let requests = inner.requests.lock().unwrap();
        assert!(
            requests.len() >= 3,
            "unsafe, explicit retry and the successor request after the receipt resumes"
        );
        assert!(
            offers(&requests[0]),
            "the maintenance round must offer compact"
        );
        assert!(
            offers(&requests[1]),
            "the same-turn retry must offer compact"
        );
        assert!(
            !offers(requests.last().unwrap()),
            "the successor is ordinary"
        );
    }

    let snapshot = service.snapshot(&strand.id).await.unwrap().unwrap();
    assert_eq!(snapshot.compacts.len(), 1, "the retry commits one Compact");
    assert_eq!(
        snapshot.compacts[0].last, last,
        "the explicit retry owns the whole history"
    );
    assert!(
        snapshot
            .turns
            .iter()
            .any(|held| held.id == successor.id
                && held.status == santi_core::turn::Status::Completed)
    );
    let calls = snapshot
        .calls
        .iter()
        .filter(|call| call.tool == "compact")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2, "both compact attempts are recorded");
    let failures = calls
        .iter()
        .filter(|call| {
            snapshot
                .results
                .iter()
                .any(|reply| reply.call == call.id && reply.error.is_some())
        })
        .count();
    assert_eq!(
        failures, 1,
        "only the rejected attempt carries an error reply"
    );
    assert!(
        snapshot
            .turns
            .iter()
            .all(|held| held.status != santi_core::turn::Status::Failed)
    );

    let balanced = service
        .audit(&strand.id)
        .await
        .unwrap()
        .unwrap()
        .pressure
        .expect("the committed compact must restore an active allowance");
    assert!(
        balanced.active <= balanced.ceiling,
        "the successor must return under the ordinary active allowance"
    );
}
