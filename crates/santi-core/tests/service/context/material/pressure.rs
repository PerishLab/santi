use super::*;

struct Limited {
    small: Arc<AtomicBool>,
}

#[async_trait]
impl Provider for Limited {
    fn metadata(&self) -> Metadata {
        let mut metadata = FakeProvider::default().metadata();
        metadata.budget = Some(santi_provider::Cap {
            bytes: if self.small.load(Ordering::SeqCst) {
                100
            } else {
                500_000
            },
            maintenance: 750_000,
            source: "test".into(),
        });
        metadata
    }
    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        FakeProvider::default().stream(request).await
    }
}

#[tokio::test]
async fn refusal() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    source(&temp, "pressure-marker");
    let small = Arc::new(AtomicBool::new(false));
    let service = Service::open(
        config(&temp),
        Arc::new(Limited {
            small: small.clone(),
        }),
    )
    .await
    .unwrap()
    .catalogued(sources(&temp))
    .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    service
        .load(Scope::Strand, &strand, command("good"))
        .await
        .unwrap();
    small.store(true, Ordering::SeqCst);
    source(&temp, "refused-marker");
    assert!(
        service
            .load(Scope::Strand, &strand, command("refused"))
            .await
            .unwrap_err()
            .contains("fixed context")
    );
    let material::View::State(state) = service
        .browse(Scope::Strand, &strand, material::Query::default())
        .await
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(state.bindings[0].operation, "good");
    service
        .drop(Scope::Strand, &strand, command("recovery"))
        .await
        .unwrap();
    let material::View::State(state) = service
        .browse(Scope::Strand, &strand, material::Query::default())
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(state.bindings.is_empty());
}

struct Maintenance {
    requests: Arc<Mutex<Vec<Request>>>,
}

#[async_trait]
impl Provider for Maintenance {
    fn metadata(&self) -> Metadata {
        FakeProvider::default().metadata()
    }
    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let round = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        let actions = match round {
            1 | 2 => vec![("query", serde_json::json!({}))],
            3 => vec![
                (
                    "material",
                    serde_json::json!({"action":"query","scope":"strand"}),
                ),
                (
                    "material",
                    serde_json::json!({"action":"drop","scope":"strand","reference":"perish:plumb"}),
                ),
                (
                    "compact",
                    serde_json::json!({"summary":"Retain the settled observation."}),
                ),
            ],
            _ => {
                let request = self.requests.lock().unwrap().last().unwrap().clone();
                return FakeProvider::default().stream(request).await;
            }
        };
        let mut events = vec![Ok(Event::Started {
            response: Some(format!("response_{round}")),
        })];
        for (index, (name, arguments)) in actions.into_iter().enumerate() {
            events.push(Ok(Event::Called(santi_provider::Call {
                response: format!("response_{round}"),
                mark: None,
                item: serde_json::json!({}),
                call: format!("call_{round}_{index}"),
                name: name.into(),
                raw: arguments.to_string(),
                arguments,
            })));
        }
        events.push(Ok(Event::Completed {
            response: Some(format!("response_{round}")),
        }));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[tokio::test]
async fn maintenance() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    source(&temp, "maintenance-marker");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let cfg = config(&temp);
    let service = Service::open(
        cfg.clone(),
        Arc::new(Maintenance {
            requests: requests.clone(),
        }),
    )
    .await
    .unwrap()
    .catalogued(sources(&temp))
    .unwrap()
    .bounded(santi_core::budget::Execution {
        timeout: 60,
        profile: "test".into(),
        rounds: 3,
        calls: 10,
        output: 100_000,
        shell: 10_000,
        lines: 100,
        feedback_after_calls: None,
        feedback_command: None,
        feedback_cwd: None,
    })
    .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    let store = santi_core::Store::open(&cfg.database).await.unwrap();
    for text in ["earlier observation", "later observation"] {
        store
            .place(santi_estate::MessageDraft {
                tag: &santi_core::tag("msg"),
                strand: &strand,
                actor: santi_core::message::Role::System,
                actor_id: "santi",
                kind: santi_core::message::Kind::Text,
                content: &santi_core::message::Content::text(text),
                state: santi_core::message::State::Fixed,
                request: false,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }
    service
        .load(Scope::Strand, &strand, command("load"))
        .await
        .unwrap();
    let posted = service
        .send(
            &strand,
            strand::Post {
                content: vec![santi_core::message::Part::Text {
                    text: "maintain".into(),
                }],
            },
        )
        .await
        .unwrap();
    let turn = accepted_turn(&service, &posted).await;
    let runtime = Probe::new(&service).completed_turn(&strand, &turn.id).await;
    let calls = runtime
        .calls
        .iter()
        .filter(|call| call.tool == "material")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert!(
            runtime
                .results
                .iter()
                .find(|r| r.call == call.id)
                .unwrap()
                .error
                .is_none()
        );
    }
    assert!(runtime.effects.is_empty());
    assert!(!runtime.compacts.is_empty());
    assert!(
        store
            .bindings(Scope::Strand, &strand)
            .await
            .unwrap()
            .bindings
            .is_empty()
    );
    let requests = requests.lock().unwrap();
    let tools = requests[2].tools.as_ref().unwrap();
    assert!(
        tools
            .iter()
            .any(|santi_provider::Tool::Function(t)| t.name == "material")
    );
    assert!(
        !tools
            .iter()
            .any(|santi_provider::Tool::Function(t)| t.name == "shell")
    );
}
