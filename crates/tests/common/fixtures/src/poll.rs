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

// Waits until the tasks in flight fall back to `baseline`, the count taken
// before the work under test started; a context's own long-lived tasks (the
// document collab timer) stay in flight, so a full drain never completes.
pub async fn settle_background(
    tasks: &systemprompt_traits::BackgroundTasks,
    baseline: usize,
    timeout: Duration,
) -> systemprompt_traits::DrainOutcome {
    let deadline = Instant::now() + timeout;
    loop {
        let in_flight = tasks.in_flight();
        if in_flight <= baseline {
            return systemprompt_traits::DrainOutcome::Drained;
        }
        if Instant::now() >= deadline {
            return systemprompt_traits::DrainOutcome::TimedOut { in_flight };
        }
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
