use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_api::services::server::metrics::install_recorder;
use systemprompt_api::services::server::runner_supervisor::{RUNNER_RESTARTS_TOTAL, supervise};
use systemprompt_identifiers::{Actor, InstanceId, UserId};
use systemprompt_traits::{BackgroundTasks, Dependencies, DrainOutcome, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy)]
enum Exit {
    Panic,
    Return,
}

struct Flaky {
    name: &'static str,
    exit: Exit,
    runs: Arc<AtomicUsize>,
}

#[async_trait]
impl RunnerService for Flaky {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn run(&self, _ctx: JobContext, _shutdown: CancellationToken) {
        self.runs.fetch_add(1, Ordering::SeqCst);
        match self.exit {
            Exit::Panic => panic!("runner {} failed", self.name),
            Exit::Return => {},
        }
    }
}

fn job_ctx() -> JobContext {
    JobContext::new(
        Actor::user(UserId::new("runner-supervisor")),
        Dependencies::new(),
    )
}

fn start(tasks: &BackgroundTasks, name: &'static str, exit: Exit) -> Arc<AtomicUsize> {
    let runs = Arc::new(AtomicUsize::new(0));
    supervise(
        tasks,
        Arc::new(Flaky {
            name,
            exit,
            runs: Arc::clone(&runs),
        }),
        job_ctx,
    );
    runs
}

async fn advance_to(elapsed: &mut Duration, target_millis: u64) {
    let target = Duration::from_millis(target_millis);
    tokio::time::sleep(target - *elapsed).await;
    *elapsed = target;
}

#[tokio::test(start_paused = true)]
async fn a_panicking_runner_is_restarted_with_a_growing_backoff_and_counted() {
    let handle = install_recorder(&InstanceId::new("supervisor-instance"), &[]).expect("recorder");
    let tasks = BackgroundTasks::new();
    let runs = start(&tasks, "supervisor-panics", Exit::Panic);
    let mut elapsed = Duration::ZERO;

    advance_to(&mut elapsed, 500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 1, "started once");
    advance_to(&mut elapsed, 1_500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 2, "restarted after one second");
    advance_to(&mut elapsed, 2_500).await;
    assert_eq!(
        runs.load(Ordering::SeqCst),
        2,
        "the second backoff is two seconds, not one"
    );
    advance_to(&mut elapsed, 3_500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 3);

    let rendered = handle.render();
    let line = rendered
        .lines()
        .find(|l| {
            l.starts_with(RUNNER_RESTARTS_TOTAL) && l.contains("runner=\"supervisor-panics\"")
        })
        .unwrap_or_else(|| panic!("restarts are counted; got: {rendered}"));
    assert!(line.trim_end().ends_with(" 3"), "{line}");
    assert_eq!(
        tasks.shutdown(Duration::from_secs(5)).await,
        DrainOutcome::Drained
    );
}

#[tokio::test(start_paused = true)]
async fn a_runner_that_returns_is_restarted() {
    let tasks = BackgroundTasks::new();
    let runs = start(&tasks, "supervisor-returns", Exit::Return);
    let mut elapsed = Duration::ZERO;

    advance_to(&mut elapsed, 500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    advance_to(&mut elapsed, 1_500).await;
    assert_eq!(
        runs.load(Ordering::SeqCst),
        2,
        "a return is not a clean stop"
    );
    assert_eq!(
        tasks.shutdown(Duration::from_secs(5)).await,
        DrainOutcome::Drained
    );
}

#[tokio::test(start_paused = true)]
async fn cancelling_the_background_tasks_stops_the_restart_loop() {
    let tasks = BackgroundTasks::new();
    let runs = start(&tasks, "supervisor-cancelled", Exit::Panic);
    let mut elapsed = Duration::ZERO;

    advance_to(&mut elapsed, 500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(
        tasks.shutdown(Duration::from_secs(5)).await,
        DrainOutcome::Drained,
        "the supervisor loop ends inside its backoff"
    );
    advance_to(&mut elapsed, 600_000).await;
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "no restart after shutdown is requested"
    );
}

struct HealthyThenPanics {
    runs: Arc<AtomicUsize>,
}

#[async_trait]
impl RunnerService for HealthyThenPanics {
    fn name(&self) -> &'static str {
        "supervisor-healthy-reset"
    }

    async fn run(&self, _ctx: JobContext, _shutdown: CancellationToken) {
        let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        if run == 2 {
            tokio::time::sleep(Duration::from_secs(360)).await;
        }
        panic!("runner failed on run {run}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_run_that_stayed_up_past_five_minutes_resets_the_backoff() {
    let tasks = BackgroundTasks::new();
    let runs = Arc::new(AtomicUsize::new(0));
    supervise(
        &tasks,
        Arc::new(HealthyThenPanics {
            runs: Arc::clone(&runs),
        }),
        job_ctx,
    );
    let mut elapsed = Duration::ZERO;

    advance_to(&mut elapsed, 1_500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 2, "restarted after one second");
    advance_to(&mut elapsed, 361_500).await;
    assert_eq!(runs.load(Ordering::SeqCst), 2, "the long run is still up");
    advance_to(&mut elapsed, 362_500).await;
    assert_eq!(
        runs.load(Ordering::SeqCst),
        3,
        "after a healthy run the backoff is one second again, not two"
    );
    assert_eq!(
        tasks.shutdown(Duration::from_secs(5)).await,
        DrainOutcome::Drained
    );
}
