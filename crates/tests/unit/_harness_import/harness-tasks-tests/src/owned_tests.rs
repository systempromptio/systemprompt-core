use std::future::pending;

use harness_tasks::{OwnedTask, TaskError};
use tokio::sync::oneshot;

struct SignalOnDrop(Option<oneshot::Sender<()>>);

impl Drop for SignalOnDrop {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            sender.send(()).unwrap();
        }
    }
}

#[tokio::test]
async fn join_hands_back_the_value() {
    let task = OwnedTask::spawn("answer", async { 42 });
    assert_eq!(task.name(), "answer");
    assert_eq!(task.join().await.unwrap(), 42);
}

#[tokio::test]
async fn dropping_the_task_aborts_it() {
    let (sender, receiver) = oneshot::channel();
    let task = OwnedTask::spawn("forever", async move {
        let _signal = SignalOnDrop(Some(sender));
        pending::<()>().await;
    });
    tokio::task::yield_now().await;
    drop(task);
    receiver.await.unwrap();
}

#[tokio::test]
async fn stop_aborts_a_pending_task() {
    let task = OwnedTask::spawn("forever", pending::<u8>());
    assert_eq!(task.stop().await, None);
}

#[tokio::test]
async fn stop_returns_the_value_of_a_finished_task() {
    let task = OwnedTask::spawn("quick", async { 7 });
    while !task.is_finished() {
        tokio::task::yield_now().await;
    }
    assert_eq!(task.stop().await, Some(7));
}

#[tokio::test]
async fn join_reports_a_panic_with_the_task_name() {
    let task = OwnedTask::spawn("doomed", async {
        panic!("boom");
    });
    let error = task.join().await.unwrap_err();
    assert!(matches!(error, TaskError::Panicked { name: "doomed", .. }));
    assert!(!error.is_cancelled());
    assert_eq!(error.task_name(), "doomed");
}

#[tokio::test]
async fn join_after_abort_reports_cancellation() {
    let task = OwnedTask::spawn("aborted", pending::<()>());
    task.abort();
    let error = task.join().await.unwrap_err();
    assert!(error.is_cancelled());
}

#[tokio::test]
async fn blocking_work_hands_back_its_value() {
    let task = OwnedTask::spawn_blocking("sum", || (1..=10).sum::<u32>());
    assert_eq!(task.join().await.unwrap(), 55);
}
