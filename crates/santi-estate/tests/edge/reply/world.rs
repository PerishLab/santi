use super::*;

pub(super) struct World {
    directory: tempfile::TempDir,
    pub(super) store: Store,
}

impl World {
    pub(super) async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::bootstrap(directory.path().join("estate.sqlite"), SUDO)
            .await
            .unwrap();
        store.seed("soul_test", FIRST).await.unwrap();
        store
            .create_strand(StrandDraft {
                tag: "strand_test",
                soul: "soul_test",
                label: None,
                parent: None,
                fork: None,
                inference: None,
                created: FIRST,
            })
            .await
            .unwrap();
        store
            .subscribe(WebhookDraft {
                name: "feishu",
                adaptor: "feishu",
                soul: "soul_test",
                strategy: "single",
                credential: "FEISHU_SECRET",
                created: FIRST,
            })
            .await
            .unwrap();
        Self { directory, store }
    }
    pub(super) async fn accept(
        &self,
        inbox: &str,
        digest: &str,
    ) -> Result<santi_estate::Accepted, String> {
        self.store
            .windowed(
                InboxDraft {
                    tag: inbox,
                    strand: "strand_test",
                    kind: message::Kind::Text,
                    content: &message::Content::text("input"),
                    source: None,
                    created: FIRST,
                },
                Acceptance {
                    replay: ReplayDraft::Webhook {
                        subscription: "feishu",
                        delivery: "event_one",
                        digest,
                    },
                    gate: 10,
                    binding: Some(&Binding {
                        adaptor: "feishu".into(),
                        context: json!({"parent":"om_input"}),
                        delay: 3000,
                    }),
                },
            )
            .await
    }
    pub(super) async fn queue(&self, owner: Owner, operation: &str) {
        self.store
            .queue(
                "inbox_test",
                Intent {
                    owner,
                    operation,
                    body: &json!({"text":operation}),
                },
            )
            .await
            .unwrap();
    }
    pub(super) async fn settle(&self, operation: &str, state: &str) {
        self.store
            .delivered(
                "inbox_test",
                Outcome {
                    operation,
                    result: &json!({"state":state}),
                    message: (state == "accepted").then_some("om_reply"),
                },
            )
            .await
            .unwrap();
    }
    pub(super) async fn restart(self) -> Self {
        drop(self.store);
        let store = Store::open(self.directory.path().join("estate.sqlite"))
            .await
            .unwrap();
        store.interrupt().await.unwrap();
        Self {
            directory: self.directory,
            store,
        }
    }
    pub(super) async fn complete(&self) {
        self.store
            .drain_turn(DrainDraft {
                turn: "turn_test",
                strand: "strand_test",
                trigger: turn::Trigger::System,
                actor: "santi",
                created: LATER,
            })
            .await
            .unwrap();
        self.store
            .complete_turn("turn_test", 2, LATER)
            .await
            .unwrap();
    }
}
