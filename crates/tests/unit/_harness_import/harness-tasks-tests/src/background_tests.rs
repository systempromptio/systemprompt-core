use std::future::pending;
use std::time::Duration;

use harness_tasks::{BackgroundTasks, ShutdownReport};
use tokio::sync::oneshot;

#[tokio::test(start_paused = true)]
async fn shutdown_cancels_and_drains_cooperative_tasks() {
    let tasks = BackgroundTasks::new();
    tasks.spawn_cancellable("watcher", |token| async move { token.cancelled().await });
    tasks.spawn_cancellable("poller", |token| async move { token.cancelled().await });
    assert_eq!(tasks.len(), 2);
    assert!(!tasks.is_shutting_down());

    let report = tasks.shutdown(Duration::from_secs(5)).await;

    assert_eq!(report, ShutdownReport::Drained);
    assert!(report.is_drained());
    assert!(tasks.is_shutting_down());
    assert!(tasks.is_empty());
}

#[tokio::test(start_paused = true)]
async fn shutdown_names_and_aborts_stragglers() {
    let tasks = BackgroundTasks::new();
    tasks.spawn("stubborn", pending::<()>());
    tasks.spawn_cancellable("polite", |token| async move { token.cancelled().await });

    let report = tasks.shutdown(Duration::from_millis(50)).await;

    assert_eq!(
        report,
        ShutdownReport::Aborted {
            stragglers: vec!["stubborn"]
        }
    );
    for _ in 0..100 {
        if tasks.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn a_panicking_task_does_not_stop_the_set_draining() {
    let tasks = BackgroundTasks::new();
    tasks.spawn("doomed", async {
        panic!("boom");
    });
    let (sender, receiver) = oneshot::channel();
    tasks.spawn("survivor", async move {
        sender.send(5).unwrap();
    });
    assert_eq!(receiver.await.unwrap(), 5);
    assert!(tasks.shutdown(Duration::from_secs(5)).await.is_drained());
}

#[tokio::test]
async fn blocking_work_runs_and_is_drained() {
    let tasks = BackgroundTasks::new();
    let (sender, receiver) = oneshot::channel();
    tasks.spawn_blocking("hash", move || {
        sender.send((1..=4).product::<u32>()).unwrap();
    });
    assert_eq!(receiver.await.unwrap(), 24);
    assert!(tasks.shutdown(Duration::from_secs(5)).await.is_drained());
}

#[tokio::test]
async fn child_tokens_are_cancelled_by_shutdown() {
    let tasks = BackgroundTasks::new();
    let token = tasks.child_token();
    let clone = tasks.clone();
    assert!(!token.is_cancelled());
    assert!(clone.shutdown(Duration::from_secs(1)).await.is_drained());
    assert!(token.is_cancelled());
    assert!(tasks.is_shutting_down());
}
