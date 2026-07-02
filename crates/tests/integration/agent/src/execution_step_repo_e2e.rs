use anyhow::Result;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{ExecutionStep, PlannedTool, StepStatus};

use crate::common::Fixture;

#[tokio::test]
async fn create_and_get_execution_step() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let step = ExecutionStep::understanding(task_id.clone());
    let sid = step.step_id.clone();
    repo.create(&step).await?;

    let fetched = repo.get(&sid).await?.expect("step exists");
    assert_eq!(fetched.task_id, task_id);
    assert_eq!(fetched.status, StepStatus::Completed);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn get_unknown_step_returns_none() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;
    use systemprompt_models::StepId;
    let sid = StepId::new();
    let result = repo.get(&sid).await?;
    assert!(result.is_none());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn list_by_task_returns_steps_in_started_order() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let s1 = ExecutionStep::understanding(task_id.clone());
    repo.create(&s1).await?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let s2 = ExecutionStep::planning(task_id.clone(), Some("plan".to_string()), None);
    repo.create(&s2).await?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let s3 = ExecutionStep::completion(task_id.clone());
    repo.create(&s3).await?;

    let list = repo.list_by_task(&task_id).await?;
    assert_eq!(list.len(), 3);
    assert!(list[0].started_at <= list[1].started_at);
    assert!(list[1].started_at <= list[2].started_at);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn list_by_task_empty_for_unknown() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;
    use systemprompt_identifiers::TaskId;
    let tid = TaskId::new("__no_task_steps_qq");
    let list = repo.list_by_task(&tid).await?;
    assert!(list.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn complete_step_transitions_to_completed() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    // Tool execution = in-progress until completed
    let step = ExecutionStep::tool_execution(task_id.clone(), "noop", serde_json::json!({}));
    let sid = step.step_id.clone();
    let started = step.started_at;
    repo.create(&step).await?;

    repo.complete_step(&sid, started, Some(serde_json::json!({"ok": true})))
        .await?;

    let fetched = repo.get(&sid).await?.expect("step");
    assert_eq!(fetched.status, StepStatus::Completed);
    assert!(fetched.completed_at.is_some());
    assert!(fetched.duration_ms.is_some());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn complete_step_without_result_does_not_change_content() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let step = ExecutionStep::tool_execution(task_id.clone(), "tool1", serde_json::json!({"a": 1}));
    let sid = step.step_id.clone();
    let started = step.started_at;
    repo.create(&step).await?;
    repo.complete_step(&sid, started, None).await?;

    let fetched = repo.get(&sid).await?.unwrap();
    assert_eq!(fetched.status, StepStatus::Completed);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn fail_step_transitions_to_failed() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let step = ExecutionStep::tool_execution(task_id.clone(), "bad", serde_json::json!({}));
    let sid = step.step_id.clone();
    let started = step.started_at;
    repo.create(&step).await?;
    repo.fail_step(&sid, started, "boom").await?;

    let fetched = repo.get(&sid).await?.expect("step");
    assert_eq!(fetched.status, StepStatus::Failed);
    assert_eq!(fetched.error_message.as_deref(), Some("boom"));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn fail_in_progress_steps_for_task_marks_outstanding_steps() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let s_inprog = ExecutionStep::tool_execution(task_id.clone(), "t", serde_json::json!({}));
    let s_inprog_id = s_inprog.step_id.clone();
    repo.create(&s_inprog).await?;

    let s_done = ExecutionStep::understanding(task_id.clone());
    let s_done_id = s_done.step_id.clone();
    repo.create(&s_done).await?;

    repo.fail_in_progress_steps_for_task(&task_id, "shutdown")
        .await?;

    let after = repo.get(&s_inprog_id).await?.unwrap();
    assert_eq!(after.status, StepStatus::Failed);
    assert!(
        after
            .error_message
            .as_deref()
            .unwrap_or("")
            .contains("shutdown")
    );

    // Completed step unaffected
    let done = repo.get(&s_done_id).await?.unwrap();
    assert_eq!(done.status, StepStatus::Completed);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn complete_planning_step_records_reasoning_and_tools() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Submitted).await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;

    let planning = ExecutionStep::planning(task_id.clone(), None, None);
    let sid = planning.step_id.clone();
    let started = planning.started_at;
    repo.create(&planning).await?;

    repo.complete_planning_step(
        &sid,
        started,
        Some("derived reasoning".to_string()),
        Some(vec![PlannedTool {
            tool_name: "do_thing".to_string(),
            arguments: serde_json::json!({}),
        }]),
    )
    .await?;

    let after = repo.get(&sid).await?.unwrap();
    assert_eq!(after.status, StepStatus::Completed);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn mcp_execution_id_exists_false_for_random_id() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = ExecutionStepRepository::new(&fx.db)?;
    let exists = repo
        .mcp_execution_id_exists(&systemprompt_identifiers::McpExecutionId::new(
            "nonexistent-mcp-exec-id-zzzzz",
        ))
        .await?;
    assert!(!exists);
    fx.cleanup().await?;
    Ok(())
}
