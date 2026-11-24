//! The request-analytics write queue: writes under the bound all run on one
//! owned writer, a write offered to a full queue is dropped and counted, and
//! a drain of the process's background work waits for every queued write.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_api::services::middleware::analytics::{AnalyticsQueue, Offered};
use systemprompt_traits::BackgroundTasks;
use tokio::sync::Notify;

fn queue(background: &BackgroundTasks, capacity: u32, writers: u32) -> AnalyticsQueue {
    AnalyticsQueue::new(
        background.clone(),
        NonZeroU32::new(capacity).expect("capacity"),
        NonZeroU32::new(writers).expect("writers"),
    )
}

#[tokio::test]
async fn every_write_under_the_bound_is_recorded() {
    let background = BackgroundTasks::new();
    let queue = queue(&background, 1024, 4);
    let written = Arc::new(AtomicUsize::new(0));
    for _ in 0..500 {
        let written = Arc::clone(&written);
        assert_eq!(
            queue.offer(async move {
                written.fetch_add(1, Ordering::Relaxed);
            }),
            Offered::Queued
        );
    }
    assert!(background.drain(Duration::from_secs(10)).await.is_drained());
    assert_eq!(written.load(Ordering::Relaxed), 500);
    assert_eq!(queue.dropped(), 0);
    assert_eq!(queue.pending(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_queue_drops_and_counts_without_waiting() {
    let background = BackgroundTasks::new();
    let queue = queue(&background, 1, 1);
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let written = Arc::new(AtomicUsize::new(0));

    let blocking = {
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        let written = Arc::clone(&written);
        async move {
            entered.notify_one();
            release.notified().await;
            written.fetch_add(1, Ordering::Relaxed);
        }
    };
    assert_eq!(queue.offer(blocking), Offered::Queued);
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the writer picks up the first write");

    let queued = Arc::clone(&written);
    assert_eq!(
        queue.offer(async move {
            queued.fetch_add(1, Ordering::Relaxed);
        }),
        Offered::Queued
    );
    let lost = Arc::clone(&written);
    assert_eq!(
        queue.offer(async move {
            lost.fetch_add(1, Ordering::Relaxed);
        }),
        Offered::Dropped
    );
    assert_eq!(queue.dropped(), 1);

    release.notify_one();
    assert!(background.drain(Duration::from_secs(10)).await.is_drained());
    assert_eq!(written.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn at_most_one_writer_task_is_alive() {
    let background = BackgroundTasks::new();
    let queue = queue(&background, 4096, 8);
    for _ in 0..2000 {
        let _ = queue.offer(tokio::task::yield_now());
        assert!(background.in_flight() <= 1, "{}", background.in_flight());
    }
    assert!(background.drain(Duration::from_secs(10)).await.is_drained());
}
