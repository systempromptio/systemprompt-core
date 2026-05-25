use anyhow::Result;
use std::sync::Arc;
use systemprompt_models::a2a::TaskState;

use crate::common::Fixture;

/// N writers race to set conflicting terminal states. Exactly one must win,
/// the rest must fail with a constraint-violation (CAS / state-machine), and
/// the persisted final status must be one of the legal terminals.
#[tokio::test]
async fn concurrent_terminal_writes_have_a_single_winner() -> Result<()> {
    let fx = Arc::new(Fixture::new().await?);
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    fx.repo
        .update_task_state(&task_id, TaskState::Working, &chrono::Utc::now())
        .await?;

    let targets = [
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Canceled,
        TaskState::Completed,
        TaskState::Failed,
    ];

    let mut handles = Vec::with_capacity(targets.len());
    for target in targets {
        let fx = Arc::clone(&fx);
        let task_id = task_id.clone();
        handles.push(tokio::spawn(async move {
            fx.repo
                .update_task_state(&task_id, target, &chrono::Utc::now())
                .await
        }));
    }

    let mut wins = 0_u32;
    let mut losses = 0_u32;
    for h in handles {
        match h.await? {
            Ok(()) => wins += 1,
            Err(_) => losses += 1,
        }
    }

    assert_eq!(wins, 1, "exactly one writer must win (got {wins})");
    assert_eq!(losses, 7);

    let status = fx.current_status(&task_id).await?;
    assert!(
        matches!(
            status.as_str(),
            "TASK_STATE_COMPLETED" | "TASK_STATE_FAILED" | "TASK_STATE_CANCELED"
        ),
        "final state must be a legal terminal, got {status}"
    );

    fx.cleanup().await?;
    Ok(())
}

/// Concurrent attempts to advance Submitted -> Working: many can compete but
/// only one CAS will succeed; the persisted state is Working either way.
#[tokio::test]
async fn concurrent_working_writes_converge_to_working() -> Result<()> {
    let fx = Arc::new(Fixture::new().await?);
    let task_id = fx.insert_task(TaskState::Submitted).await?;

    let mut handles = Vec::new();
    for _ in 0..16 {
        let fx = Arc::clone(&fx);
        let task_id = task_id.clone();
        handles.push(tokio::spawn(async move {
            fx.repo
                .update_task_state(&task_id, TaskState::Working, &chrono::Utc::now())
                .await
        }));
    }

    let mut hard_failures = 0_u32;
    for h in handles {
        if let Err(e) = h.await? {
            let s = format!("{e:?}");
            // CAS contention is the expected loser path; anything else is a bug.
            assert!(
                s.contains("stale task update") || s.contains("invalid task state transition"),
                "unexpected failure: {s}"
            );
            hard_failures = hard_failures.saturating_add(0);
        }
    }
    let _ = hard_failures;

    let status = fx.current_status(&task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");

    fx.cleanup().await?;
    Ok(())
}
