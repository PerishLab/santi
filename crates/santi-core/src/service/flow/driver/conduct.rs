use super::*;
use crate::service::{Service, interrupt::Control};
use std::future::Future;
use std::pin::Pin;

impl Service {
    pub(in crate::service::flow) fn conduct(
        &self,
        strand: String,
        turn: String,
        control: Control,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            match self.run(&strand, &turn, &control).await {
                Err(failure) => self.bury(&strand, &turn, failure).await,
                Ok(finish) if finish.settled => {}
                Ok(finish) => match self.halted(&control) {
                    Some(cause) => self.bury(&strand, &turn, Failure::stopped(cause, "")).await,
                    None => self.land(&strand, &turn, finish).await,
                },
            }
            self.noticed(&turn).await;
            self.release(&turn);
            self.unsettle(&turn);
            self.poke(&strand, "strand_send", None, "turn_completion_poke")
                .await;
            self.relieve(&strand).await;
            self.forget(&turn);
        })
    }
}
