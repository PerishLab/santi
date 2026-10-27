use super::super::support::*;
use futures_util::StreamExt;
use santi_core::{
    environ::Scope,
    material,
    service::{self, Service},
    strand,
};
use std::sync::atomic::{AtomicBool, Ordering};

mod continuity;
mod pressure;
mod rounds;
mod wire;

struct Fixture {
    temp: tempfile::TempDir,
    service: Service,
    provider: FakeProvider,
    strand: String,
}

impl Fixture {
    async fn open() -> Self {
        let temp = tempfile::tempdir().unwrap();
        bootstrap(&temp).await;
        source(&temp, "first-marker");
        let provider = FakeProvider::default();
        let service = Service::open(config(&temp), Arc::new(Observed(provider.clone())))
            .await
            .unwrap()
            .catalogued(sources(&temp))
            .unwrap();
        let strand = service.weave().await.unwrap().strand.id;
        Self {
            temp,
            service,
            provider,
            strand,
        }
    }

    async fn load(&self, scope: Scope, operation: &str) -> material::Applied {
        let owner = if scope == Scope::Soul {
            "soul_default"
        } else {
            &self.strand
        };
        self.service
            .load(scope, owner, command(operation))
            .await
            .unwrap()
    }

    async fn send(&self, text: &str) -> santi_core::stream::Snapshot {
        let posted = self
            .service
            .send(
                &self.strand,
                strand::Post {
                    content: vec![santi_core::message::Part::Text { text: text.into() }],
                },
            )
            .await
            .unwrap();
        let turn = accepted_turn(&self.service, &posted).await;
        Probe::new(&self.service)
            .completed_turn(&self.strand, &turn.id)
            .await
    }

    async fn prompt(&self) -> String {
        self.service
            .material(
                &self.strand,
                material::Request {
                    kind: material::Kind::SystemPrompt,
                },
            )
            .await
            .unwrap()
            .text
    }
}

fn command(operation: &str) -> material::Command {
    material::Command {
        reference: "perish:plumb".into(),
        operation: Some(operation.into()),
    }
}

fn config(temp: &tempfile::TempDir) -> service::Config {
    service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    }
}

fn sources(temp: &tempfile::TempDir) -> material::Sources {
    [(
        "perish".into(),
        material::Source {
            root: temp.path().join("sources").display().to_string(),
            format: material::Format::Skill,
        },
    )]
    .into()
}

fn source(temp: &tempfile::TempDir, marker: &str) {
    let root = temp.path().join("sources/plumb");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("SKILL.md"),
        format!("---\nname: plumb\ndescription: Operate Plumb.\n---\n{marker}\n"),
    )
    .unwrap();
}

#[tokio::test]
async fn projection() {
    let fixture = Fixture::open().await;
    assert!(!fixture.prompt().await.contains("first-marker"));
    let loaded = fixture.load(Scope::Soul, "shared").await;
    fixture.load(Scope::Strand, "local").await;
    assert_eq!(fixture.prompt().await.matches("first-marker").count(), 1);
    source(&fixture.temp, "second-marker");
    assert!(fixture.prompt().await.contains("first-marker"));
    let runtime = fixture.send("hello").await;
    assert_eq!(runtime.contexts.len(), 1);
    let observed = &runtime.contexts[0];
    assert!(observed.thinking.is_some());
    assert_eq!(observed.materials.len(), 1);
    assert_eq!(observed.materials[0].origins.len(), 2);
    assert_eq!(
        observed.materials[0].revision,
        loaded.change.current.unwrap()
    );
    let request = fixture.provider.requests.lock().unwrap()[0].clone();
    assert!(request.instructions.unwrap().contains("first-marker"));
    fixture
        .service
        .drop(Scope::Strand, &fixture.strand, command("drop_local"))
        .await
        .unwrap();
    assert!(fixture.prompt().await.contains("first-marker"));
    fixture
        .service
        .drop(Scope::Soul, "soul_default", command("drop_shared"))
        .await
        .unwrap();
    assert!(!fixture.prompt().await.contains("first-marker"));
    std::fs::remove_file(fixture.temp.path().join("sources/plumb/SKILL.md")).unwrap();
    let material::View::Blob(retained) = fixture
        .service
        .browse(
            Scope::Strand,
            &fixture.strand,
            material::Query {
                context: Some(observed.id.clone()),
                reference: Some("perish:plumb".into()),
                blob: Some("SKILL.md".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(retained.text.contains("first-marker"));
}

#[tokio::test]
async fn refresh() {
    let fixture = Fixture::open().await;
    let first = fixture.load(Scope::Strand, "first").await;
    source(&fixture.temp, "new-marker");
    let refreshed = fixture.load(Scope::Strand, "refresh").await;
    assert_ne!(first.change.current, refreshed.change.current);
    assert!(fixture.prompt().await.contains("new-marker"));
    std::fs::write(
        fixture.temp.path().join("sources/plumb/SKILL.md"),
        "bad front matter",
    )
    .unwrap();
    assert!(
        fixture
            .service
            .load(Scope::Strand, &fixture.strand, command("failed"))
            .await
            .is_err()
    );
    assert!(fixture.prompt().await.contains("new-marker"));
    let restarted = Service::open(config(&fixture.temp), Arc::new(FakeProvider::default()))
        .await
        .unwrap();
    let rendered = restarted
        .material(
            &fixture.strand,
            material::Request {
                kind: material::Kind::SystemPrompt,
            },
        )
        .await
        .unwrap();
    assert!(rendered.text.contains("new-marker"));
}

struct Observed(FakeProvider);

#[async_trait]
impl Provider for Observed {
    fn metadata(&self) -> Metadata {
        self.0.metadata()
    }
    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let response = self.0.stream(request).await?;
        Ok(Box::pin(
            stream::once(async {
                Ok(Event::Started {
                    response: Some("observed".into()),
                })
            })
            .chain(response),
        ))
    }
}
