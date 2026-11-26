use santi_core::render;
use santi_core::service::{self, Service};
use std::{fs, sync::Arc};

use async_trait::async_trait;
use futures_util::stream;
use santi_core::{SOULSPACE, STRANDSPACE, soulward, strandward};
use santi_provider::{Metadata, Provider, Streaming};

#[derive(Clone)]
struct Fake;

#[async_trait]
impl Provider for Fake {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("fake-provider"),
            model: "fake-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, _: santi_provider::Request) -> Result<Streaming, String> {
        Ok(Box::pin(stream::empty()))
    }
}

#[tokio::test]
async fn renders() {
    let harness = Harness::open().await;
    harness.soul("---\nplain: value\n---\n# Soul");
    harness.strand("# Strand");

    let text = harness.prompt().await.text;

    assert!(text.contains("[santi]"));
    assert!(text.contains("santi keeps souls and runs their strands."));
    assert!(text.contains("[santi-meta]"));
    assert!(!text.contains("channel: santi"));
    assert!(text.contains("soul: soul_default"));
    assert!(text.contains("strand: "));
    assert!(!text.contains("soul_name"));
    assert!(text.contains(&format!("{} supplies [santi-soul]", soulward())));
    assert!(text.contains(&format!("{} supplies [santi-strand]", strandward())));
    assert!(text.contains(&format!(
        "save versions into {SOULSPACE} or {STRANDSPACE} as needed."
    )));
    assert!(text.contains("<system_message> blocks report runtime facts"));
    assert!(text.contains("[santi-soul]"));
    assert!(text.contains("[santi-strand]"));
    assert!(text.contains(&format!("source: {}", soulward())));
    assert!(text.contains(&format!("source: {}", strandward())));
    assert!(text.contains("content:\n---\nplain: value\n---\n# Soul"));
    assert!(text.contains("content:\n# Strand"));
    assert!(!text.contains("hint:"));
    assert!(!text.contains("@soul"));
    assert!(!text.contains("@strand"));
}

#[tokio::test]
async fn leaves() {
    let harness = Harness::open().await;
    harness.soul("---\nplain: value\n---\n# Soul");

    let text = harness.prompt().await.text;

    assert!(text.contains("content:\n---\nplain: value\n---\n# Soul"));
    assert!(!text.contains("hint:"));
}

#[tokio::test]
async fn overrides() {
    let harness = Harness::open().await;
    harness.constitution("my own physics, hot-edited");

    let text = harness.prompt().await.text;

    assert!(text.contains("[santi]\nmy own physics, hot-edited"));
    assert!(!text.contains("santi keeps souls and runs their strands."));
}

#[tokio::test]
async fn fallback() {
    let harness = Harness::open().await;

    let text = harness.prompt().await.text;

    assert!(text.contains("You are a soul — a cyber-individual."));
    assert!(!text.to_lowercase().contains("secretary"));
}

#[tokio::test]
async fn projects() {
    let harness = Harness::open().await;
    let memory = format!("# Memory\n{}\nSOURCE_TAIL", "界".repeat(90_000));
    harness.soul(&memory);

    let text = harness.prompt().await.text;

    assert!(text.contains("kind: soul_memory_projection"));
    assert!(text.contains("allowance_bytes: 32768"));
    assert!(!text.contains("SOURCE_TAIL"));
    let source = fs::read_to_string(harness.runtime.join("souls/soul_default/memory/MEMORY.md"))
        .expect("read source memory");
    assert_eq!(
        source, memory,
        "projection must never rewrite source memory"
    );
}

#[tokio::test]
async fn excludes() {
    let harness = Harness::open().await;
    let first = harness
        .service
        .evented(santi_core::GENESIS, "stim:operator", "hello".to_string())
        .await
        .expect("first strand");
    let second = harness
        .service
        .evented(
            santi_core::GENESIS,
            "github:ops:issue:PerishCode/santi#1",
            "hello".to_string(),
        )
        .await
        .expect("second strand");
    let santi_core::ingest::Outcome::Accepted {
        receipt: first_receipt,
    } = first
    else {
        panic!("first ingest rejected");
    };
    let santi_core::ingest::Outcome::Accepted {
        receipt: second_receipt,
    } = second
    else {
        panic!("second ingest rejected");
    };
    let alpha = harness.render(&first_receipt.strand).await.text;
    let beta = harness.render(&second_receipt.strand).await.text;
    assert!(!alpha.contains("stim:operator"));
    assert!(!beta.contains("github:ops:issue:PerishCode/santi#1"));
}

#[path = "prompt/skill.rs"]
mod skill;

#[path = "prompt/identity.rs"]
mod identity;

struct Harness {
    _temp: tempfile::TempDir,
    service: Service,
    strand: String,
    runtime: std::path::PathBuf,
}

impl Harness {
    async fn open() -> Self {
        let temp = tempfile::tempdir().expect("temp dir");
        let runtime = temp.path().join("runtime");
        santi_core::Store::bootstrap(
            temp.path().join("santi.sqlite"),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .await
        .expect("bootstrap");
        let service = Service::open(
            "fixture/model/execution",
            service::Config {
                database: temp.path().join("santi.sqlite").display().to_string(),
                runtime: runtime.display().to_string(),
                execution: temp.path().join("execution").display().to_string(),
                bind: Some("127.0.0.1:0".to_string()),
                constitution: None,
                environment: Default::default(),
            },
            Arc::new(Fake),
        )
        .await
        .expect("open service");
        let strand = service.weave().await.expect("create strand").strand.id;
        Self {
            _temp: temp,
            service,
            strand,
            runtime,
        }
    }

    fn soul(&self, text: &str) {
        let path = self
            .runtime
            .join("souls")
            .join("soul_default")
            .join("memory");
        fs::create_dir_all(&path).expect("create soul dir");
        fs::write(path.join("MEMORY.md"), text).expect("write soul");
    }

    fn strand(&self, text: &str) {
        let path = self
            .runtime
            .join("strands")
            .join(&self.strand)
            .join("memory");
        fs::create_dir_all(&path).expect("create strand dir");
        fs::write(path.join("MEMORY.md"), text).expect("write strand");
    }

    fn constitution(&self, text: &str) {
        fs::create_dir_all(&self.runtime).expect("create runtime dir");
        fs::write(self.runtime.join("constitution.md"), text).expect("write constitution");
    }

    async fn prompt(&self) -> render::View {
        self.render(&self.strand).await
    }

    async fn render(&self, strand: &str) -> render::View {
        self.service
            .render(
                strand,
                render::Request {
                    kind: render::Kind::Prompt,
                },
            )
            .await
            .expect("system prompt material")
    }
}
