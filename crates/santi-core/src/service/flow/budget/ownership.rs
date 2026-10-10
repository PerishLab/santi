use crate::service::Service;
use std::sync::Arc;
struct Unused;
#[async_trait::async_trait]
impl santi_provider::Provider for Unused {
    fn metadata(&self) -> santi_provider::Metadata {
        santi_provider::Metadata {
            provider: Arc::from("unused"),
            model: "unused".into(),
            budget: None,
        }
    }
    async fn stream(
        &self,
        _: santi_provider::Request,
    ) -> Result<santi_provider::Streaming, String> {
        panic!("ownership experiment uses no provider")
    }
}
const NOW: &str = "2026-10-10T00:00:00.000Z";
const LATER: &str = "2026-10-10T00:01:00.000Z";
#[tokio::test]
async fn isolates() {
    for maintaining in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("estate.sqlite");
        let store = santi_estate::Store::bootstrap(
            &database,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .await
        .unwrap();
        drop(store);
        let service = Service::open(
            crate::service::Config {
                database: database.display().to_string(),
                runtime: temp.path().join("runtime").display().to_string(),
                execution: temp.path().join("execution").display().to_string(),
                bind: None,
                constitution: None,
                environment: Default::default(),
            },
            Arc::new(Unused),
        )
        .await
        .unwrap()
        .bounded(crate::budget::Execution {
            profile: "research".into(),
            rounds: 8,
            calls: 32,
            output: 10000,
            shell: 1000,
            feedback_after_calls: None,
            feedback_command: None,
            feedback_cwd: None,
        })
        .unwrap();
        service
            .store
            .create_strand(santi_estate::StrandDraft {
                tag: "strand_research",
                soul: crate::GENESIS,
                label: None,
                parent: None,
                fork: None,
                created: NOW,
            })
            .await
            .unwrap();
        service
            .store
            .create_turn(santi_estate::TurnDraft {
                tag: "turn_old",
                strand: "strand_research",
                trigger: crate::turn::Trigger::System,
                source: None,
                from: 0,
                created: NOW,
            })
            .await
            .unwrap();
        if maintaining {
            service
                .settling("strand_research", "turn_old", 8)
                .await
                .unwrap();
        }

        service
            .store
            .finish_turn(santi_estate::CompletionDraft {
                turn: "turn_old",
                reply: None,
                provider: "unused",
                model: "unused",
                response: None,
                occurred: NOW,
            })
            .await
            .unwrap();
        service
            .store
            .accept_inbox(
                santi_estate::InboxDraft {
                    tag: "inbox_new",
                    strand: "strand_research",
                    kind: crate::message::Kind::SantiSystem,
                    content: &crate::message::Content::text("next"),
                    source: None,
                    created: LATER,
                },
                500,
            )
            .await
            .unwrap();
        let opening = service
            .store
            .drain_turn(santi_estate::DrainDraft {
                turn: "turn_new",
                strand: "strand_research",
                trigger: crate::turn::Trigger::System,
                source: None,
                actor: crate::SYSTEM,
                created: LATER,
            })
            .await
            .unwrap();
        assert!(matches!(opening, santi_estate::Opening::Started(_)));
        service
            .settling(
                "strand_research",
                "turn_new",
                if maintaining { 1 } else { 8 },
            )
            .await
            .unwrap();
        if maintaining {
            assert!(service.settlement("turn_old").is_some());
            assert!(service.settlement("turn_new").is_none());
            let offered = serde_json::to_value(
                service
                    .offers("strand_research", Some("turn_new"))
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(offered[0]["Function"]["name"], "shell");
            let observed =
                serde_json::to_value(service.offered("strand_research").await.unwrap()).unwrap();
            assert_eq!(observed[0]["Function"]["name"], "shell");
        } else {
            assert!(
                service
                    .settlement("turn_new")
                    .unwrap()
                    .contains("turn: turn_new")
            );
            service.unsettle("turn_old");
            assert!(service.settlement("turn_new").is_some());
            let offered = serde_json::to_value(
                service
                    .offers("strand_research", Some("turn_new"))
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(offered[0]["Function"]["name"], "compact");
            let observed =
                serde_json::to_value(service.offered("strand_research").await.unwrap()).unwrap();
            assert_eq!(observed[0]["Function"]["name"], "compact");
        }
    }
}
