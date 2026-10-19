use std::time::{Duration, Instant};

use super::super::Service;

impl Service {
    pub async fn watch(&self) {
        let mut next = Instant::now();
        while !self.closing() {
            if let Err(error) = self.sweep().await {
                eprintln!("santi: job attention scan failed: {error}");
            }
            self.reconcile_alarms().await;
            next = self.pace(next).await;
            self.rouse().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
