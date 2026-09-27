use std::time::{Duration, Instant};

use super::super::Service;

pub(in crate::service) mod clock;
mod notice;

impl Service {
    pub async fn watch(&self) {
        let mut next = Instant::now();
        while !self.closing() {
            if let Err(error) = self.sweep().await {
                eprintln!("santi: job attention scan failed: {error}");
            }
            if let Err(error) = clock::ring(self).await {
                eprintln!("santi: soul clock failed: {error}");
            }
            next = self.pace(next).await;
            self.rouse().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
