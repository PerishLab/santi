use super::{Service, attention, terminal};

impl Service {
    pub(in crate::service) async fn sweep(&self) -> Result<(), String> {
        for record in self.store.active_jobs().await? {
            let id = record.job.id.clone();
            let result = match self.refresh(record).await {
                Ok(record) => attention::capture(self, record).await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                eprintln!("santi: job attention failed job={id} detail={error}");
            }
        }
        for record in self.store.unnotified(64).await? {
            let id = record.job.id.clone();
            if let Err(error) = terminal::finish(self, record).await {
                eprintln!("santi: terminal job notification failed job={id} detail={error}");
            }
        }
        Ok(())
    }
}
