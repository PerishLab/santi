use super::{Service, attention};

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
        Ok(())
    }
}
