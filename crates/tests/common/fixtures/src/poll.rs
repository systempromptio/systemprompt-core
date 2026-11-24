//! Bounded polling for conditions that settle asynchronously.

use std::future::Future;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

// A fixed quota window is aligned to the Unix epoch, so two charges a test
// expects in one bucket land in two when the clock crosses a boundary between
// them. Waits out the current window when fewer than `headroom` remain, so the
// test starts at least `headroom` before the next boundary.
pub async fn await_window_headroom(window_seconds: u64, headroom: Duration) {
    let window = Duration::from_secs(window_seconds);
    assert!(headroom < window, "headroom must be shorter than the window");
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after the Unix epoch");
    let elapsed = u64::try_from(since_epoch.as_nanos() % window.as_nanos())
        .expect("a window position fits in u64 nanoseconds");
    let left = window - Duration::from_nanos(elapsed);
    if left < headroom {
        tokio::time::sleep(left + Duration::from_millis(50)).await;
    }
}
