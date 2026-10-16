//! A node that drains in order leaves its gateway journal empty: the
//! recovery task makes one final settlement pass when the shutdown drain
//! cancels it, so a request cut by the drain is settled as failed before the
//! process exits instead of waiting an hour for another replica's orphan sweep.

use std::time::Duration;

use systemprompt_gateway::audit::journal;
use systemprompt_traits::BackgroundTasks;

use super::gateway_pipeline::admitted_receipt_fixture;

async fn latest_status(database: &systemprompt_test_fixtures::DisposableDb) -> String {
    let pool = database.test_pool().await;
    sqlx::query_scalar("SELECT status FROM ai_requests ORDER BY created_at DESC LIMIT 1")
        .fetch_one(pool.write_pool().as_ref())
        .await
        .expect("admitted request row")
}

#[tokio::test]
async fn the_shutdown_flush_settles_a_cut_request_and_empties_the_journal() -> anyhow::Result<()> {
    let (database, repositories, _state_dir, receipt) =
        admitted_receipt_fixture("gateway_journal_shutdown_flush").await?;
    let settlement = repositories.settlement();
    assert_eq!(journal::pending(&settlement.journal).await?, 1);

    journal::flush_on_shutdown(&settlement).await;

    assert_eq!(journal::pending(&settlement.journal).await?, 0);
    assert!(!receipt.exists());
    assert_ne!(latest_status(&database).await, "pending");
    database.drop_now().await;
    Ok(())
}

#[tokio::test]
async fn a_settlement_pass_publishes_the_receipts_left_on_disk() -> anyhow::Result<()> {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let _local = metrics::set_default_local_recorder(&recorder);
    let (database, repositories, _state_dir, _receipt) =
        admitted_receipt_fixture("gateway_journal_metrics").await?;
    let settlement = repositories.settlement();

    let settled = journal::recover(&settlement).await?;

    let values: Vec<(String, DebugValue)> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .map(|(key, _, _, value)| (key.key().name().to_owned(), value))
        .collect();
    let value = |name: &str| {
        values
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    };
    assert_eq!(settled, 1);
    assert_eq!(
        value(journal::JOURNAL_RECEIPTS),
        Some(&DebugValue::Gauge(0.0.into()))
    );
    assert_eq!(value(journal::JOURNAL_SETTLE_FAILURES_TOTAL), None);
    assert_eq!(value(journal::JOURNAL_RECOVERY_FAILURES_TOTAL), None);
    database.drop_now().await;
    Ok(())
}

#[tokio::test]
async fn a_failed_settlement_is_counted_and_the_receipt_stays_on_the_gauge() -> anyhow::Result<()> {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let _local = metrics::set_default_local_recorder(&recorder);
    let (database, repositories, _state_dir, receipt) =
        admitted_receipt_fixture("gateway_journal_metrics_fail").await?;
    let pool = database.test_pool().await;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_journal_drain() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'journal drain settlement fault'; END $$; \
         CREATE TRIGGER reject_journal_drain BEFORE UPDATE ON ai_requests \
         FOR EACH ROW \
         EXECUTE FUNCTION reject_journal_drain()",
    )
    .execute(pool.write_pool().as_ref())
    .await?;

    let settled = journal::recover(&repositories.settlement()).await?;

    let values: Vec<(String, DebugValue)> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .map(|(key, _, _, value)| (key.key().name().to_owned(), value))
        .collect();
    let value = |name: &str| {
        values
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    };
    assert_eq!(settled, 0);
    assert!(
        receipt.exists(),
        "a receipt that did not settle is retained"
    );
    assert_eq!(
        value(journal::JOURNAL_SETTLE_FAILURES_TOTAL),
        Some(&DebugValue::Counter(1))
    );
    assert_eq!(
        value(journal::JOURNAL_RECEIPTS),
        Some(&DebugValue::Gauge(1.0.into()))
    );
    database.drop_now().await;
    Ok(())
}

#[tokio::test]
async fn draining_background_tasks_runs_the_final_settlement_pass() -> anyhow::Result<()> {
    let (database, repositories, _state_dir, receipt) =
        admitted_receipt_fixture("gateway_journal_drain_pass").await?;
    let settlement = repositories.settlement();
    let tasks = BackgroundTasks::new();
    journal::spawn_recovery(settlement.clone(), &tasks);

    let outcome = tasks.shutdown(Duration::from_secs(10)).await;

    assert!(
        outcome.is_drained(),
        "the flush must finish inside the grace"
    );
    assert_eq!(journal::pending(&settlement.journal).await?, 0);
    assert!(!receipt.exists());
    assert_ne!(latest_status(&database).await, "pending");
    database.drop_now().await;
    Ok(())
}
