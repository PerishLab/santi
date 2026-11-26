use super::*;

pub(crate) struct Probe<'a> {
    service: &'a Service,
}

impl<'a> Probe<'a> {
    pub(crate) fn new(service: &'a Service) -> Self {
        Self { service }
    }

    async fn snapshot(&self, strand: &str) -> santi_core::stream::Snapshot {
        self.service
            .snapshot(strand)
            .await
            .expect("runtime snapshot")
            .expect("strand runtime")
    }

    pub(crate) async fn settled(&self, strand: &str) -> santi_core::stream::Snapshot {
        eventually(self.service, "a turn to complete", async || {
            self.snapshot(strand)
                .await
                .turns
                .iter()
                .any(|turn| turn.status == santi_core::turn::Status::Completed)
                .then_some(())
        })
        .await;
        self.snapshot(strand).await
    }

    pub(crate) async fn completed(&self, strand: &str, turn: &str) -> santi_core::stream::Snapshot {
        eventually(self.service, "the turn to complete", async || {
            self.snapshot(strand)
                .await
                .turns
                .iter()
                .any(|held| held.id == turn && held.status == santi_core::turn::Status::Completed)
                .then_some(())
        })
        .await;
        self.snapshot(strand).await
    }

    pub(crate) async fn failed(&self, strand: &str, turn: &str) -> santi_core::stream::Snapshot {
        eventually(self.service, "the turn to fail", async || {
            let runtime = self.snapshot(strand).await;
            runtime
                .turns
                .iter()
                .any(|held| held.id == turn && held.status == santi_core::turn::Status::Failed)
                .then_some(runtime)
        })
        .await
    }
}
