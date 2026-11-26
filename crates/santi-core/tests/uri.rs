use std::path::PathBuf;

use santi_core::{SOULSPACE, STRANDSPACE, housed, parsed, soulward, strandward};

#[test]
fn builds() {
    assert_eq!(soulward(), "soul://MEMORY.md");
    assert_eq!(strandward(), "strand://MEMORY.md");
    assert_eq!(housed(SOULSPACE, "notes/today.md"), "soul://notes/today.md");
    assert_eq!(housed(STRANDSPACE, "/todo.md"), "strand://todo.md");
}

#[test]
fn parses() {
    let soul = parsed(SOULSPACE).expect("soul root");
    assert_eq!(soul.root, santi_core::workspace::Root::Soul);
    assert_eq!(soul.path, PathBuf::new());

    let strand = parsed("strand://notes/today.md").expect("strand path");
    assert_eq!(strand.root, santi_core::workspace::Root::Strand);
    assert_eq!(strand.path, PathBuf::from("notes/today.md"));
}

#[test]
fn aliases() {
    assert_eq!(
        parsed("@soul").expect_err("old soul alias"),
        "unsupported workspace alias: @soul; use soul:// or strand://"
    );
    assert_eq!(
        parsed("@strand").expect_err("old strand alias"),
        "unsupported workspace alias: @strand; use soul:// or strand://"
    );
}

#[test]
fn invalid() {
    assert_eq!(
        parsed("file://tmp").expect_err("unknown scheme"),
        "unsupported workspace uri: file://tmp"
    );
    assert_eq!(
        parsed("relative/path").expect_err("relative path"),
        "cwd must use soul:// or strand://"
    );
    assert_eq!(
        parsed("soul://../secret").expect_err("escape"),
        "workspace uri cannot escape soul://"
    );
}

const SCENARIO: &str = "fixture/model/execution";

#[allow(dead_code, unused_imports)]
#[path = "service/support/mod.rs"]
mod support;

use santi_core::service::{self, Service};
use santi_core::{message, soul, strand};
use santi_provider::Call;
use serde_json::json;
use std::path::Path;
use support::*;

#[derive(Clone, Default)]
struct Cwd {
    cwd: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

#[async_trait]
impl Provider for Cwd {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("cwd-provider"),
            model: "cwd-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let round = {
            let mut requests = self.requests.lock().unwrap();
            let round = requests.len();
            requests.push(request);
            round
        };
        if round == 0 {
            let arguments = json!({
                "cwd": self.cwd,
                "command": "pwd"
            });
            Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: "response_cwd".to_string(),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_cwd".to_string(),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some("response_cwd".to_string()),
                }),
            ])))
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("done".to_string())),
                Ok(Event::Completed {
                    response: Some("response_done".to_string()),
                }),
            ])))
        }
    }
}

struct World {
    service: Service,
    soul: String,
    strand: String,
    runtime: PathBuf,
}

fn memory(runtime: &Path, kind: &str, id: &str) -> PathBuf {
    runtime.join(kind).join(id).join("memory/MEMORY.md")
}

async fn world(temp: &tempfile::TempDir, cwd: &str) -> World {
    bootstrap(temp).await;
    let runtime = temp.path().join("runtime");
    let provider = Arc::new(Cwd {
        cwd: cwd.to_string(),
        requests: Arc::new(Mutex::new(Vec::new())),
    });
    let service = Service::open(
        SCENARIO,
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: runtime.display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .expect("open service");
    let soul = service
        .awaken(soul::Draft {
            memory: Some("keep this memoir\n".to_string()),
            inference: Default::default(),
        })
        .await
        .expect("awaken soul");
    let strand = service.seat(&soul.id).await.expect("seat soul").strand;
    World {
        service,
        soul: soul.id,
        strand: strand.id,
        runtime,
    }
}

async fn once(world: &World) -> santi_core::stream::Snapshot {
    let response = world
        .service
        .send(
            &world.strand,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "run a shell command".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    Probe::new(&world.service)
        .completed(&world.strand, &accepted(&world.service, &response).await.id)
        .await
}

#[tokio::test]
async fn reserved() {
    let temp = tempfile::tempdir().expect("temp dir");
    let world = world(&temp, "strand://MEMORY.md").await;
    let path = memory(&world.runtime, "strands", &world.strand);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("memory dir");
    std::fs::write(&path, "strand notes\n").expect("journal");
    let before = std::fs::read(&path).expect("journal before");
    let prior = std::fs::read(memory(&world.runtime, "souls", &world.soul)).expect("memoir before");
    let result = once(&world).await;
    let error = result.results[0].error.as_deref().expect("shell error");
    assert!(
        error.contains("reserved memory file"),
        "unexpected error: {error}"
    );
    assert!(path.is_file());
    assert!(!path.is_dir());
    assert_eq!(std::fs::read(&path).expect("journal after"), before);
    assert_eq!(
        std::fs::read(memory(&world.runtime, "souls", &world.soul)).expect("memoir after"),
        prior
    );
    world
        .service
        .snapshot(&world.strand)
        .await
        .expect("snapshot")
        .expect("strand remains readable");
    world
        .service
        .audit(&world.strand)
        .await
        .expect("budget audit")
        .expect("budget remains readable");
}

#[tokio::test]
async fn soul() {
    let temp = tempfile::tempdir().expect("temp dir");
    let world = world(&temp, "soul://MEMORY.md").await;
    let path = memory(&world.runtime, "souls", &world.soul);
    let before = std::fs::read(&path).expect("memoir before");
    let result = once(&world).await;
    let error = result.results[0].error.as_deref().expect("shell error");
    assert!(
        error.contains("reserved memory file"),
        "unexpected error: {error}"
    );
    assert!(path.is_file());
    assert!(!path.is_dir());
    assert_eq!(std::fs::read(&path).expect("memoir after"), before);
}

#[tokio::test]
async fn ordinary() {
    let temp = tempfile::tempdir().expect("temp dir");
    let world = world(&temp, "strand://work").await;
    let result = once(&world).await;
    assert!(result.results[0].error.is_none());
    let path = world
        .runtime
        .join("strands")
        .join(&world.strand)
        .join("memory/work");
    assert!(path.is_dir());
    assert!(memory(&world.runtime, "souls", &world.soul).is_file());
}

#[tokio::test]
async fn existing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let world = world(&temp, "strand://notes.txt").await;
    let path = world
        .runtime
        .join("strands")
        .join(&world.strand)
        .join("memory/notes.txt");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("memory dir");
    std::fs::write(&path, "notes").expect("file cwd");
    let result = once(&world).await;
    let error = result.results[0].error.as_deref().expect("shell error");
    assert!(
        error.contains("exists and is not a directory"),
        "unexpected error: {error}"
    );
    assert!(path.is_file());
    assert!(!path.is_dir());
}
