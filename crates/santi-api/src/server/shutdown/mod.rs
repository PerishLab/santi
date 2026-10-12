use santi_core::service::Service;
use std::{future::Future, future::IntoFuture, time::Duration};
use tokio::sync::watch;
use tokio::time::Instant;

#[derive(Clone)]
struct Window(watch::Receiver<Option<Instant>>);

impl Window {
    async fn elapsed(&self) {
        let mut receiver = self.0.clone();
        loop {
            let deadline = *receiver.borrow_and_update();
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(deadline).await;
                return;
            }
            if receiver.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }
}

pub(super) async fn serve(
    service: Service,
    listener: tokio::net::TcpListener,
    grace: Duration,
    signal: impl Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    let (closing, receiver) = watch::channel(None);
    let window = Window(receiver);
    let shutdown = {
        let service = service.clone();
        let closing = closing.clone();
        async move {
            signal.await;
            println!(
                "santi-api: shutdown signal received — quiescing (no new turns; grace={}s)",
                grace.as_secs()
            );
            service.quiesce(grace);
            closing.send_replace(Some(Instant::now() + grace));
            println!("santi-api: waiting for HTTP server shutdown");
        }
    };
    let drainer = service.clone();
    let recovery = service.clone();
    let mut watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };
    let server = axum::serve(listener, super::routes::router(service))
        .with_graceful_shutdown(shutdown)
        .into_future();
    tokio::pin!(server);
    let mut resume = tokio::spawn(async move { recovery.resume().await });
    let serving = async {
        tokio::select! {
            result = &mut server => result.map_err(|error| error.to_string()),
            recovered = &mut resume => match recovered {
                Ok(Ok(())) => server.await.map_err(|error| error.to_string()),
                Ok(Err(error)) => Err(error),
                Err(error) => Err(error.to_string()),
            },
        }
    };
    tokio::pin!(serving);
    let result = tokio::select! {
        result = &mut serving => result,
        _ = window.elapsed() => {
            println!("santi-api: HTTP shutdown grace elapsed");
            Ok(())
        }
    };
    if !drainer.closing() {
        drainer.quiesce(Duration::ZERO);
        closing.send_replace(Some(Instant::now()));
    }
    println!("santi-api: waiting for auxiliary watcher shutdown");
    tokio::select! {
        result = &mut watcher => result.map_err(|error| error.to_string())?,
        _ = window.elapsed() => {
            println!("santi-api: auxiliary watcher shutdown grace elapsed");
        }
    }
    println!("santi-api: waiting for core turn settlement");
    drainer.drain().await;
    println!("santi-api: drained; exiting");
    result
}

#[cfg(test)]
mod tests;
