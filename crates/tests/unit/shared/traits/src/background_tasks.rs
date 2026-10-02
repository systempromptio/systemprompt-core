use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use systemprompt_traits::{BackgroundTasks, DrainOutcome, OwnedTask};
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
