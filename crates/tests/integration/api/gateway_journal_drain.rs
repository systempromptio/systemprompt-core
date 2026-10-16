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
