use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_traits::{BackgroundTasks, DrainOutcome, OwnedTask, TaskError};
use tokio::sync::oneshot;

const DRAIN: Duration = Duration::from_secs(30);

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

#[tokio::test(start_paused = true)]
async fn drain_waits_for_spawned_work() {
    let tasks = BackgroundTasks::new();
    let (release, gate) = oneshot::channel::<()>();
    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    tasks.spawn("gated_write", async move {
        gate.await.unwrap();
        done.store(true, Ordering::SeqCst);
    });

    assert_eq!(tasks.in_flight(), 1);
    assert_eq!(
        tasks.drain(Duration::from_secs(1)).await,
        DrainOutcome::TimedOut { in_flight: 1 }
    );
    assert!(!finished.load(Ordering::SeqCst));

    release.send(()).unwrap();
    assert_eq!(tasks.drain(DRAIN).await, DrainOutcome::Drained);
    assert!(finished.load(Ordering::SeqCst));
    assert_eq!(tasks.in_flight(), 0);
}

#[tokio::test]
async fn drain_reopens_so_later_work_is_still_tracked() {
    let tasks = BackgroundTasks::new();
    assert_eq!(tasks.drain(DRAIN).await, DrainOutcome::Drained);

    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    tasks.spawn("after_drain", async move {
        tokio::task::yield_now().await;
        done.store(true, Ordering::SeqCst);
    });

    assert!(tasks.drain(DRAIN).await.is_drained());
    assert!(finished.load(Ordering::SeqCst));
    assert!(!tasks.is_shutting_down());
}

#[tokio::test(start_paused = true)]
async fn shutdown_cancels_a_cancellable_loop() {
    let tasks = BackgroundTasks::new();
    tasks.spawn_cancellable("heartbeat", |cancel| async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = interval.tick() => {},
            }
        }
    });
    assert_eq!(tasks.shutdown(DRAIN).await, DrainOutcome::Drained);
    assert!(tasks.is_shutting_down());
    assert!(tasks.cancellation_token().is_cancelled());
    assert_eq!(tasks.in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn shutdown_does_not_interrupt_one_shot_work() {
    let tasks = BackgroundTasks::new();
    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    tasks.spawn("audit_write", async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        done.store(true, Ordering::SeqCst);
    });

    assert_eq!(tasks.shutdown(DRAIN).await, DrainOutcome::Drained);
    assert!(finished.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_panicking_task_is_reported_under_the_spawners_subscriber() {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let tasks = BackgroundTasks::new();
    tasks.spawn("exploding_write", async {
        panic!("boom");
    });

    assert_eq!(tasks.drain(DRAIN).await, DrainOutcome::Drained);
    let logged = capture.text();
    assert!(logged.contains("Background task panicked"), "{logged}");
    assert!(logged.contains("exploding_write"), "{logged}");

    let (tx, rx) = oneshot::channel();
    tasks.spawn("still_serving", async move {
        tx.send(()).unwrap();
    });
    assert!(tasks.drain(DRAIN).await.is_drained());
    rx.await.unwrap();
}

#[tokio::test]
async fn dropping_an_owned_task_aborts_it() {
    let (held, observed) = oneshot::channel::<()>();
    let task = OwnedTask::spawn("listener", async move {
        let _held = held;
        std::future::pending::<()>().await;
    });
    assert_eq!(task.name(), "listener");
    assert!(!task.is_finished());

    drop(task);

    assert!(
        observed.await.is_err(),
        "the aborted task released its state"
    );
}

#[tokio::test]
async fn owned_task_join_returns_the_output() {
    let task = OwnedTask::spawn("compute", async { 41 + 1 });

    assert_eq!(task.join().await.unwrap(), 42);
}

#[tokio::test]
async fn owned_task_join_reports_a_panic() {
    let task = OwnedTask::<()>::spawn("exploding", async {
        panic!("boom");
    });

    assert!(task.join().await.unwrap_err().is_panic());
}

#[tokio::test]
async fn abort_and_join_stops_a_pending_task() {
    let task = OwnedTask::<()>::spawn("forever", std::future::pending());

    assert_eq!(task.abort_and_join().await, None);
}

#[tokio::test(start_paused = true)]
async fn shutdown_or_abort_drains_cooperative_tasks() {
    let tasks = BackgroundTasks::new();
    tasks.spawn_cancellable("watcher", |token| async move { token.cancelled().await });
    tasks.spawn_cancellable("poller", |token| async move { token.cancelled().await });
    assert_eq!(tasks.in_flight(), 2);
    assert!(!tasks.is_shutting_down());

    let outcome = tasks.shutdown_or_abort(Duration::from_secs(5)).await;

    assert_eq!(outcome, DrainOutcome::Drained);
    assert!(tasks.is_shutting_down());
    assert_eq!(tasks.in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn shutdown_or_abort_counts_and_aborts_stragglers() {
    let tasks = BackgroundTasks::new();
    let (held, observed) = oneshot::channel::<()>();
    tasks.spawn("stubborn", async move {
        let _held = held;
        std::future::pending::<()>().await;
    });
    tasks.spawn_cancellable("polite", |token| async move { token.cancelled().await });

    let outcome = tasks.shutdown_or_abort(Duration::from_millis(50)).await;

    assert_eq!(outcome, DrainOutcome::TimedOut { in_flight: 1 });
    assert!(
        observed.await.is_err(),
        "the aborted straggler released its state"
    );
    assert!(tasks.drain(DRAIN).await.is_drained());
    assert_eq!(tasks.in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn shutdown_leaves_stragglers_running() {
    let tasks = BackgroundTasks::new();
    let (release, gate) = oneshot::channel::<()>();
    let finished = Arc::new(AtomicBool::new(false));
    let done = Arc::clone(&finished);
    tasks.spawn("slow_write", async move {
        gate.await.unwrap();
        done.store(true, Ordering::SeqCst);
    });

    assert_eq!(
        tasks.shutdown(Duration::from_millis(50)).await,
        DrainOutcome::TimedOut { in_flight: 1 }
    );
    release.send(()).unwrap();
    assert!(tasks.drain(DRAIN).await.is_drained());
    assert!(finished.load(Ordering::SeqCst));
}

#[tokio::test]
async fn blocking_work_runs_and_is_drained() {
    let tasks = BackgroundTasks::new();
    let (sender, receiver) = oneshot::channel();
    tasks.spawn_blocking("hash", move || {
        sender.send((1..=4).product::<u32>()).unwrap();
    });
    assert_eq!(receiver.await.unwrap(), 24);
    assert!(tasks.shutdown_or_abort(DRAIN).await.is_drained());
}

#[tokio::test]
async fn panicking_blocking_work_is_still_drained() {
    let tasks = BackgroundTasks::new();
    tasks.spawn_blocking("doomed", || panic!("boom"));
    assert!(tasks.drain(DRAIN).await.is_drained());
    assert_eq!(tasks.in_flight(), 0);
}

#[tokio::test]
async fn cancellation_tokens_are_cancelled_by_shutdown() {
    let tasks = BackgroundTasks::new();
    let token = tasks.cancellation_token();
    let clone = tasks.clone();
    assert!(!token.is_cancelled());
    assert!(
        clone
            .shutdown_or_abort(Duration::from_secs(1))
            .await
            .is_drained()
    );
    assert!(token.is_cancelled());
    assert!(tasks.is_shutting_down());
}

#[tokio::test]
async fn abort_and_join_returns_the_value_of_a_finished_task() {
    let task = OwnedTask::spawn("quick", async { 7 });
    while !task.is_finished() {
        tokio::task::yield_now().await;
    }
    assert_eq!(task.abort_and_join().await, Some(7));
}

#[tokio::test]
async fn join_named_hands_back_the_value() {
    let task = OwnedTask::spawn("answer", async { 42 });
    assert_eq!(task.join_named().await.unwrap(), 42);
}

#[tokio::test]
async fn join_named_reports_a_panic_with_the_task_name() {
    let task = OwnedTask::<()>::spawn("doomed", async {
        panic!("boom");
    });
    let error = task.join_named().await.unwrap_err();
    assert!(matches!(error, TaskError::Panicked { name: "doomed", .. }));
    assert!(!error.is_cancelled());
    assert_eq!(error.task_name(), "doomed");
    assert_eq!(error.to_string(), "task doomed panicked");
}

#[tokio::test]
async fn join_named_after_abort_reports_cancellation() {
    let task = OwnedTask::spawn("aborted", std::future::pending::<()>());
    task.abort();
    let error = task.join_named().await.unwrap_err();
    assert!(error.is_cancelled());
    assert_eq!(error.task_name(), "aborted");
}

#[tokio::test]
async fn blocking_work_hands_back_its_value() {
    let task = OwnedTask::spawn_blocking("sum", || (1..=10).sum::<u32>());
    assert_eq!(task.join_named().await.unwrap(), 55);
}
