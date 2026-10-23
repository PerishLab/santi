use super::super::support::*;
use santi_core::{Store, compact, message, service, strand};

const BUDGET: usize = 200_000;
const CEILING: i64 = BUDGET as i64 - 90_000;

struct Plain(FakeProvider);

#[async_trait]
impl Provider for Plain {
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
        self.0.stream(request).await
    }
}

struct Opened {
    _temp: tempfile::TempDir,
    service: service::Service,
    store: Store,
    strand: String,
}

async fn opened() -> Opened {
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
    let service = service::Service::open(config.clone(), Arc::new(Plain(FakeProvider::default())))
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    let store = Store::open(&config.database).await.unwrap();
    Opened {
        _temp: temp,
        service,
        store,
        strand,
    }
}

impl Opened {
    async fn active(&self) -> i64 {
        let audit = self.service.audit(&self.strand).await.unwrap().unwrap();
        let settled = audit
            .slots
            .map_or(0, |slots| slots.held.iter().map(|slot| slot.bytes).sum());
        audit.estimate.total - settled
    }

    async fn fill(&self, tag: &str, share: i64) {
        let missing = CEILING * share / 100 - self.active().await;
        self.store
            .place(santi_estate::MessageDraft {
                tag,
                strand: &self.strand,
                actor: message::Role::System,
                actor_id: santi_core::SYSTEM,
                kind: message::Kind::Text,
                content: &message::Content::text("x".repeat(missing as usize)),
                state: message::State::Fixed,
                request: false,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }

    async fn send(&self, text: &str) {
        self.service
            .send(
                &self.strand,
                strand::Post {
                    content: vec![message::Part::Text { text: text.into() }],
                },
            )
            .await
            .unwrap();
    }

    async fn reminded(&self, count: usize) -> Vec<String> {
        for _ in 0..300 {
            let reminders = self
                .service
                .snapshot(&self.strand)
                .await
                .unwrap()
                .unwrap()
                .messages
                .into_iter()
                .map(|message| message.text)
                .filter(|text| text.contains("kind: compact_reminder"))
                .collect::<Vec<_>>();
            if reminders.len() >= count {
                return reminders;
            }
            sleep(Duration::from_millis(20)).await;
        }
        panic!("expected {count} compact reminders");
    }
}

fn field(reminder: &str, name: &str) -> String {
    reminder
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("reminder lacks {name}"))
        .to_string()
}

#[tokio::test]
async fn measures() {
    let held = opened().await;
    held.fill("msg_history", 80).await;
    let audit = held.service.audit(&held.strand).await.unwrap().unwrap();
    assert!(audit.estimate.input + audit.estimate.instructions < BUDGET as i64 / 2);
    held.send("go").await;
    let reminder = held.reminded(1).await.remove(0);
    assert_eq!(field(&reminder, "band"), "firm");
    assert_eq!(field(&reminder, "estimator"), "provider_json_bytes_v1");
    assert_eq!(
        field(&reminder, "active_ceiling_bytes"),
        CEILING.to_string()
    );
    let active = field(&reminder, "active_bytes").parse::<i64>().unwrap();
    let placed = held.active().await;
    assert!(active < placed && placed - active < reminder.len() as i64 + 200);
    assert_eq!(
        field(&reminder, "headroom_bytes"),
        (CEILING - active).to_string()
    );
}

#[tokio::test]
async fn unweighed() {
    let held = opened().await;
    let before = held.service.audit(&held.strand).await.unwrap().unwrap();
    let next = held.store.strand(&held.strand).await.unwrap().unwrap().next;
    let created = santi_core::now();
    held.store
        .create_turn(santi_estate::TurnDraft {
            tag: "turn_thought",
            strand: &held.strand,
            trigger: santi_core::turn::Trigger::System,
            source: None,
            from: next,
            created: &created,
        })
        .await
        .unwrap();
    held.store
        .create_thinking(santi_estate::ThinkingDraft {
            tag: "thinking_thought",
            turn: "turn_thought",
            response: Some("response_thought"),
            created: &created,
        })
        .await
        .unwrap();
    held.store
        .update_thinking("thinking_thought", None, Some("a summary"), &created)
        .await
        .unwrap();
    let projected = santi_core::provider_input(&held.store, &held.strand)
        .await
        .unwrap();
    assert!(!format!("{projected:?}").contains("a summary"));
    let after = held.service.audit(&held.strand).await.unwrap().unwrap();
    assert_eq!(after.estimate.items, before.estimate.items);
    assert_eq!(after.estimate.total, before.estimate.total);
}

#[tokio::test]
async fn rearms() {
    let held = opened().await;
    held.fill("msg_first", 60).await;
    held.send("first").await;
    assert_eq!(field(&held.reminded(1).await[0], "band"), "soft");
    held.service
        .exec(
            &held.strand,
            compact::Exec {
                first: Some("msg_first".into()),
                last: Some("msg_first".into()),
                from: None,
                to: None,
                summary: "a settled history".into(),
                absorb: Vec::new(),
                capsule: None,
                dry: false,
            },
        )
        .await
        .unwrap();
    held.fill("msg_second", 60).await;
    held.send("second").await;
    let reminders = held.reminded(2).await;
    assert_eq!(field(&reminders[1], "band"), "soft");
}
