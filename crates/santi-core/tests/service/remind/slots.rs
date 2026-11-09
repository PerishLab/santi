use super::super::super::support::*;
use super::{BUDGET, field};
use santi_core::{Store, compact, message, service, strand};

#[derive(Clone)]
struct Bounded(FakeProvider);

#[async_trait]
impl Provider for Bounded {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("plain"),
            model: "plain".into(),
            budget: Some(santi_provider::Cap {
                bytes: BUDGET,
                maintenance: BUDGET * 2,
                source: "test".into(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let index = {
            let mut requests = self.0.requests.lock().unwrap();
            let index = requests.len();
            requests.push(request);
            index
        };
        let response = format!("response_round_{index}");
        if index % 2 == 0 {
            let arguments = serde_json::json!({"command": "true"});
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(santi_provider::Call {
                    response: response.clone(),
                    mark: None,
                    item: serde_json::json!({"type": "function_call"}),
                    call: format!("call_round_{index}"),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some(response),
                }),
            ])));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("hi from runtime".to_string())),
            Ok(Event::Completed {
                response: Some(response),
            }),
        ])))
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    service: service::Service,
    store: Store,
    strand: String,
    provider: FakeProvider,
}

async fn opened() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let provider = FakeProvider::default();
    let service = service::Service::open(config.clone(), Arc::new(Bounded(provider.clone())))
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    let store = Store::open(&config.database).await.unwrap();
    Fixture {
        _temp: temp,
        service,
        store,
        strand,
        provider,
    }
}

impl Fixture {
    async fn fixed(&self, tag: &str, text: &str) {
        self.store
            .place(santi_estate::MessageDraft {
                tag,
                strand: &self.strand,
                actor: message::Role::System,
                actor_id: santi_core::SYSTEM,
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                state: message::State::Fixed,
                request: false,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }

    async fn exchange(&self, text: &str) {
        let posted = self
            .service
            .send(
                &self.strand,
                strand::Post {
                    content: vec![message::Part::Text {
                        text: text.to_string(),
                    }],
                },
            )
            .await
            .unwrap();
        let turn = accepted_turn(&self.service, &posted).await;
        Probe::new(&self.service)
            .completed_turn(&self.strand, &turn.id)
            .await;
    }

    fn tools(&self) -> Vec<String> {
        let requests = self.provider.requests.lock().unwrap();
        let request = requests.last().expect("provider request");
        request
            .tools
            .as_ref()
            .expect("tools")
            .iter()
            .map(|tool| match tool {
                santi_provider::Tool::Function(tool) => tool.name.clone(),
            })
            .collect()
    }

    async fn reminders(&self) -> Vec<String> {
        self.service
            .snapshot(&self.strand)
            .await
            .unwrap()
            .unwrap()
            .messages
            .into_iter()
            .map(|message| message.text)
            .filter(|text| text.contains("kind: compact_reminder"))
            .collect()
    }

    async fn reminded(&self, count: usize) -> Vec<String> {
        for _ in 0..300 {
            let reminders = self.reminders().await;
            if reminders.len() >= count {
                return reminders;
            }
            sleep(Duration::from_millis(20)).await;
        }
        panic!("expected {count} compact reminders");
    }
}

fn range(first: &str, last: &str, summary: &str) -> compact::Exec {
    compact::Exec {
        first: Some(first.to_string()),
        last: Some(last.to_string()),
        from: None,
        to: None,
        summary: summary.to_string(),
        absorb: Vec::new(),
        capsule: None,
        dry: false,
    }
}

fn merge(first: &str, second: &str, summary: &str) -> compact::Exec {
    compact::Exec {
        first: None,
        last: None,
        from: None,
        to: None,
        summary: summary.to_string(),
        absorb: vec![first.to_string(), second.to_string()],
        capsule: None,
        dry: false,
    }
}

#[tokio::test]
async fn slots() {
    let held = opened().await;
    for index in 0..12 {
        held.fixed(
            &format!("msg_slot_{index:02}"),
            &format!("settled slot message {index:02}"),
        )
        .await;
    }
    let ids = (0..12)
        .map(|index| format!("msg_slot_{index:02}"))
        .collect::<Vec<_>>();

    let mut slots = Vec::new();
    for index in 0..4 {
        let report = held
            .service
            .exec(
                &held.strand,
                range(
                    &ids[index * 2],
                    &ids[index * 2 + 1],
                    &format!("slot {index} summary"),
                ),
            )
            .await
            .unwrap();
        assert!(report.absorbed.is_empty());
        slots.push(report.compact);
    }
    let four = held.service.audit(&held.strand).await.unwrap().unwrap();
    let occupancy = four.slots.as_ref().expect("slots");
    assert_eq!(occupancy.held.len(), 4);
    assert_eq!(occupancy.free, 1);
    held.exchange("four slots observation").await;
    let tools = held.tools();
    assert!(tools.iter().any(|name| name == "shell"), "{tools:?}");
    assert!(tools.iter().any(|name| name == "query"), "{tools:?}");
    assert!(
        held.reminders().await.is_empty(),
        "4/5 below other bands must not emit the full-slot reminder"
    );

    let fifth = held
        .service
        .exec(&held.strand, range(&ids[8], &ids[9], "slot four summary"))
        .await
        .unwrap();
    slots.push(fifth.compact);
    let full = held.service.audit(&held.strand).await.unwrap().unwrap();
    let occupancy = full.slots.as_ref().expect("slots");
    assert_eq!(occupancy.held.len(), 5);
    assert_eq!(occupancy.count, 5);
    assert_eq!(occupancy.free, 0);
    held.exchange("five slots observation").await;
    let tools = held.tools();
    assert!(tools.iter().any(|name| name == "shell"), "{tools:?}");
    assert!(tools.iter().any(|name| name == "query"), "{tools:?}");
    let reminders = held.reminded(1).await;
    assert_eq!(reminders.len(), 1);
    assert_eq!(field(&reminders[0], "band"), "slots");
    assert_eq!(field(&reminders[0], "slots_held"), "5");
    assert_eq!(field(&reminders[0], "slots_ceiling"), "5");
    assert!(reminders[0].contains("Every compact slot is occupied"));
    assert!(reminders[0].contains("Merge two or more contiguous occupied Compact IDs"));
    assert!(reminders[0].contains("santi compact exec"));

    held.exchange("five slots observation again").await;
    assert_eq!(
        held.reminders().await.len(),
        1,
        "the same full state must not repeat the reminder"
    );

    held.service
        .exec(&held.strand, merge(&slots[0], &slots[1], "merged slots"))
        .await
        .unwrap();
    let merged = held.service.audit(&held.strand).await.unwrap().unwrap();
    assert_eq!(merged.slots.as_ref().expect("slots").held.len(), 4);
    held.exchange("after merge observation").await;
    assert_eq!(held.reminders().await.len(), 1);

    held.service
        .exec(
            &held.strand,
            range(&ids[10], &ids[11], "slot four again summary"),
        )
        .await
        .unwrap();
    let again = held.service.audit(&held.strand).await.unwrap().unwrap();
    assert_eq!(again.slots.as_ref().expect("slots").held.len(), 5);
    held.exchange("five slots after merge observation").await;
    let reminders = held.reminded(2).await;
    assert_eq!(
        reminders.len(),
        2,
        "filling the slot again rearms the reminder"
    );
    assert_eq!(field(&reminders[1], "band"), "slots");
}
