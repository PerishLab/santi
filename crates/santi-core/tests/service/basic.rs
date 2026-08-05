use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{message, soul, strand};

#[path = "../../src/service/face/publication.rs"]
mod publication;

#[tokio::test]
async fn seats() {
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
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");

    let strand = service.seat(&soul.id).await.expect("seat soul").strand;

    assert_eq!(strand.soul, soul.id);
    assert!(strand.memory.is_empty());
    assert!(strand.parent.is_none());
    assert!(strand.fork.is_none());
    assert!(
        service
            .strand(&strand.id)
            .await
            .expect("read strand")
            .expect("created strand")
            .messages
            .is_empty()
    );
    assert_eq!(
        service.seat("soul_missing").await.unwrap_err(),
        "soul not found"
    );
}

#[tokio::test]
async fn publication() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let runtime = temp.path().join("runtime");
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: runtime.display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(FakeProvider::default()),
    )
    .await
    .expect("open service");
    let memory = "  exact identity\n留住末尾空白 \n\n";
    let soul = service
        .awaken(soul::Draft {
            memory: Some(memory.to_string()),
        })
        .await
        .expect("awaken soul");
    let path = runtime
        .join("souls")
        .join(&soul.id)
        .join("memory")
        .join("MEMORY.md");
    assert_eq!(std::fs::read(path).expect("read memoir"), memory.as_bytes());
    assert!(
        service
            .souls()
            .await
            .expect("list souls")
            .iter()
            .any(|listed| listed.id == soul.id)
    );
}

#[tokio::test]
async fn visibility() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let runtime = temp.path().join("blocked");
    std::fs::write(&runtime, "not a directory").expect("block runtime directory");
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: runtime.display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(FakeProvider::default()),
    )
    .await
    .expect("open service");
    let before = service.souls().await.expect("souls before").len();
    assert!(
        service
            .awaken(soul::Draft {
                memory: Some("must remain exact".to_string()),
            })
            .await
            .is_err()
    );
    assert_eq!(service.souls().await.expect("souls after").len(), before);
}

#[test]
fn truncation() {
    let temp = tempfile::tempdir().expect("temp dir");
    publication::publish(temp.path(), "soul_test", b"published identity").expect("publish memoir");
    let memory = temp.path().join("souls").join("soul_test").join("memory");
    let memoir = memory.join("MEMORY.md");
    assert!(publication::memoir(&memory, b"different identity").is_err());
    assert_eq!(
        std::fs::read(&memoir).expect("retained memoir"),
        b"published identity"
    );
    assert!(!memory.join(".MEMORY.md.staged").exists());
}

#[tokio::test]
async fn sends() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(FakeProvider::default());
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider.clone(),
    )
    .await
    .expect("open service");

    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");
    assert_ne!(soul.id, santi_core::GENESIS);
    let strand = service.seat(&soul.id).await.expect("seat soul").strand;
    let response = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hello provider".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");

    assert_eq!(
        response
            .message
            .as_ref()
            .expect("driven synchronously")
            .text,
        "hello provider"
    );
    assert_eq!(
        accepted_turn(&response).status,
        santi_core::turn::Status::Running
    );
    let runtime = Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&response).id)
        .await;
    let reply = runtime
        .messages
        .iter()
        .find(|message| message.text == "hi from runtime")
        .expect("fixed assistant reply");
    assert_eq!(reply.message.actor, soul.id);

    let requests = provider.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].model, "fake-model");
    assert_eq!(requests[0].input.len(), 1);
    match &requests[0].input[0] {
        Item::Message { role, content } => {
            assert_eq!(role, "user");
            assert_eq!(content, "hello provider");
        }
        other => panic!("expected text message, got {other:?}"),
    }
    let instructions = requests[0]
        .instructions
        .as_deref()
        .expect("runtime instructions");
    assert!(instructions.contains("[santi]"));
    assert!(instructions.contains(
        "santi is an agent runtime: a container that keeps souls and runs their strands."
    ));
    assert!(instructions.contains("[santi-meta]"));
    assert!(instructions.contains(&format!("soul: {}", soul.id)));
    assert!(instructions.contains("strand: "));
    assert!(!instructions.contains("channel: santi"));
    assert!(!instructions.contains("soul_name"));
    assert!(instructions.contains("[santi-soul]"));
    assert!(instructions.contains("[santi-strand]"));
    assert!(instructions.contains(&format!(
        "{} will always be displayed in [santi-soul].",
        soulward()
    )));
    assert!(instructions.contains(&format!(
        "{} will always be displayed in [santi-strand].",
        strandward()
    )));
    assert!(instructions.contains(&format!(
        "These files have no internal version history; save backups into {SOULSPACE} or {STRANDSPACE} if needed."
    )));
    assert!(
        instructions
            .contains("<system_message> blocks describe Santi runtime facts in this strand.")
    );
    assert!(instructions.contains(
        "They are part of your context, not user speech or your natural-language reply."
    ));
    assert!(
        instructions
            .contains("Read them as strand facts about the workspace, runtime, or provider flow.")
    );
    assert!(instructions.contains(&format!("source: {}", soulward())));
    assert!(instructions.contains(&format!("source: {}", strandward())));
    assert!(!instructions.contains("hint:"));
    assert!(!instructions.contains("@soul"));
    assert!(!instructions.contains("@strand"));
    assert!(!instructions.contains("<santi-runtime>"));
    assert!(!instructions.contains("<santi-tools>"));
    let tools = requests[0].tools.as_ref().expect("tools");
    let tool_names = tools
        .iter()
        .map(|tool| match tool {
            santi_provider::Tool::Function(tool) => tool.name.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_names, vec!["shell"]);
    let tool_descriptions = tools
        .iter()
        .map(|tool| match tool {
            santi_provider::Tool::Function(tool) => {
                format!("{} {}", tool.description, tool.parameters)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(tool_descriptions.contains("Run a short, bounded shell command."));
    assert!(tool_descriptions.contains("santi job create <DESCRIPTION> <COMMAND>"));
    assert!(tool_descriptions.contains("Never keep this synchronous shell open"));
    assert!(tool_descriptions.contains(&soulward()));
    assert!(tool_descriptions.contains(&strandward()));
    assert!(!tool_descriptions.contains("@soul"));
    assert!(!tool_descriptions.contains("@strand"));

    let detail = service
        .strand(&strand.id)
        .await
        .expect("load detail")
        .expect("strand");
    assert_eq!(detail.messages.len(), 2);
    assert_eq!(runtime.turns.len(), 1);

    let store = santi_core::Store::open(temp.path().join("santi.sqlite"))
        .await
        .expect("open estate");
    let mut traces = Vec::new();
    for _ in 0..50 {
        traces = store
            .traces("turn", &accepted_turn(&response).id)
            .await
            .expect("turn traces");
        if !traces.is_empty() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(traces.len(), 1, "async trace writer must persist the turn");
    assert_eq!(traces[0].name, "turn");
}
