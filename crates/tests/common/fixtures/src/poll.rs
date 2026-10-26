//! Bounded polling for conditions that settle asynchronously.

use std::future::Future;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(25);

pub async fn eventually<F, Fut>(what: &str, timeout: Duration, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if check().await {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what} did not hold within {timeout:?}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime")
        .block_on(future)
}
