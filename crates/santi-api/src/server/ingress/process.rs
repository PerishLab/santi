use radicula_lock::{Factory, Gate, Policy};
use std::sync::OnceLock;

pub(super) fn gate(profile: &str) -> Result<Gate, String> {
    static FACTORY: OnceLock<Factory> = OnceLock::new();
    FACTORY
        .get_or_init(Factory::memory)
        .gate(profile, Policy::EXCLUSIVE)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn profiles() {
        let first = gate("feedback-test").unwrap();
        let second = gate("feedback-test").unwrap();
        let other = gate("feedback-other").unwrap();
        let active = Arc::new(AtomicUsize::new(0));
        let a = run(first, active.clone());
        let b = run(second, active);
        let (a, b) = tokio::join!(a, b);
        a.unwrap();
        b.unwrap();
        let held = gate("feedback-test")
            .unwrap()
            .acquire(radicula_lock::Wait::immediate())
            .unwrap();
        assert!(other.acquire(radicula_lock::Wait::immediate()).is_ok());
        assert!(
            gate("feedback-test")
                .unwrap()
                .acquire(radicula_lock::Wait::immediate())
                .is_err()
        );
        drop(held);
        assert!(
            gate("feedback-test")
                .unwrap()
                .acquire(radicula_lock::Wait::immediate())
                .is_ok()
        );
    }

    async fn run(gate: Gate, active: Arc<AtomicUsize>) -> Result<(), String> {
        gate.tokio()
            .guard(|| async {
                assert_eq!(active.fetch_add(1, Ordering::SeqCst), 0);
                tokio::task::yield_now().await;
                assert_eq!(active.fetch_sub(1, Ordering::SeqCst), 1);
            })
            .await
            .map_err(|e| e.to_string())
    }
}
