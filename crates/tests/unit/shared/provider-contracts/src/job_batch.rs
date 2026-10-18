//! Tests for `run_batched`: it stops on a short batch or at the deadline,
//! propagates a failing batch, and reports a spent budget in the job result.

use std::cell::Cell;
use std::time::{Duration, Instant};

use systemprompt_provider_contracts::{BatchPlan, BatchedRun, run_batched};

fn plan(full_batch: u64) -> BatchPlan {
    BatchPlan {
        deadline: Instant::now() + Duration::from_secs(60),
        full_batch,
    }
}

#[tokio::test]
async fn a_short_batch_completes_the_run() {
    let sizes = [100_u64, 100, 40, 100];
    let calls = Cell::new(0_usize);
    let run = run_batched(plan(100), || {
        let size = sizes[calls.get()];
        calls.set(calls.get() + 1);
        async move { Ok::<_, std::io::Error>(size) }
    })
    .await
    .expect("run");
    assert_eq!(
        run,
        BatchedRun {
            processed: 240,
            batches: 3,
            complete: true,
        }
    );
}

#[tokio::test]
async fn a_full_batch_of_one_stops_only_on_an_empty_batch() {
    let sizes = [3_u64, 1, 0];
    let calls = Cell::new(0_usize);
    let run = run_batched(plan(1), || {
        let size = sizes[calls.get()];
        calls.set(calls.get() + 1);
        async move { Ok::<_, std::io::Error>(size) }
    })
    .await
    .expect("run");
    assert_eq!(run.batches, 3);
    assert_eq!(run.processed, 4);
    assert!(run.complete);
}

#[tokio::test]
async fn a_passed_deadline_runs_no_batch_and_reports_more_remaining() {
    let plan = BatchPlan {
        deadline: Instant::now(),
        full_batch: 10,
    };
    let run = run_batched(plan, || async { Ok::<_, std::io::Error>(10) })
        .await
        .expect("run");
    assert_eq!(run.batches, 0);
    assert!(!run.complete);
    let result = run.into_job_result();
    assert!(result.success);
    assert!(result.message.is_some_and(|m| m.contains("more remain")));
}

#[tokio::test]
async fn a_failing_batch_fails_the_run() {
    let error = run_batched(plan(10), || async {
        Err::<u64, _>(std::io::Error::other("batch failed"))
    })
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "batch failed");
}

#[test]
fn a_complete_run_reports_its_stats_without_a_message() {
    let result = BatchedRun {
        processed: 7,
        batches: 2,
        complete: true,
    }
    .into_job_result();
    assert_eq!(result.items_processed, Some(7));
    assert!(result.message.is_none());
}
