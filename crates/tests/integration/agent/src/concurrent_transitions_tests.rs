use anyhow::Result;
use std::sync::Arc;
use systemprompt_models::a2a::TaskState;

use crate::common::Fixture;

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
            let result = fx
                .repo
                .update_task_state(&task_id, target, &chrono::Utc::now())
                .await;
            (target, result)
        }));
    }

    let mut outcomes = Vec::with_capacity(8);
    for h in handles {
        outcomes.push(h.await?);
    }

    let status = fx.current_status(&task_id).await?;
    let final_state: TaskState = status.parse().expect("final state parses");
    assert!(
        matches!(
            final_state,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled
        ),
        "final state must be a legal terminal, got {status}"
    );

    for (target, result) in outcomes {
        if target == final_state {
            assert!(
                result.is_ok(),
                "writer targeting winning terminal {target:?} must succeed, got {result:?}",
            );
        } else {
            assert!(
                result.is_err(),
                "writer targeting losing terminal {target:?} must fail, got {result:?}",
            );
        }
    }

    fx.cleanup().await?;
    Ok(())
}

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
