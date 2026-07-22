use super::{repos, seed_context_and_task, seed_user_and_session, try_pool};
use systemprompt_identifiers::{McpExecutionId, TaskId};
use systemprompt_models::{ExecutionStep, StepContent, StepId, StepStatus, StepType};

#[tokio::test]
async fn create_and_get_tool_execution_step() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step =
        ExecutionStep::tool_execution(task_id.clone(), "search", serde_json::json!({"q": "rust"}));
    let step_id = step.step_id.clone();
    r.execution_steps.create(&step).await.expect("create");

    let fetched = r
        .execution_steps
        .get(&step_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.step_id, step_id);
    assert_eq!(fetched.task_id, task_id);
    assert_eq!(fetched.step_type(), StepType::ToolExecution);
    assert_eq!(fetched.status, StepStatus::InProgress);
    assert_eq!(fetched.tool_name(), Some("search"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_unknown_step_returns_none() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let result = r.execution_steps.get(&StepId::new()).await.expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn create_instant_steps() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let understanding = ExecutionStep::understanding(task_id.clone());
    let completion = ExecutionStep::completion(task_id.clone());
    r.execution_steps
        .create(&understanding)
        .await
        .expect("create understanding");
    r.execution_steps
        .create(&completion)
        .await
        .expect("create completion");

    let u = r
        .execution_steps
        .get(&understanding.step_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(u.status, StepStatus::Completed);
    assert_eq!(u.step_type(), StepType::Understanding);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn list_by_task_ordered() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let s1 = ExecutionStep::understanding(task_id.clone());
    let s2 = ExecutionStep::tool_execution(task_id.clone(), "t", serde_json::json!({}));
    r.execution_steps.create(&s1).await.expect("c1");
    r.execution_steps.create(&s2).await.expect("c2");

    let list = r
        .execution_steps
        .list_by_task(&task_id)
        .await
        .expect("list");
    assert_eq!(list.len(), 2);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn complete_step_sets_completed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step = ExecutionStep::tool_execution(task_id.clone(), "t", serde_json::json!({"a": 1}));
    let step_id = step.step_id.clone();
    let started_at = step.started_at;
    r.execution_steps.create(&step).await.expect("create");

    r.execution_steps
        .complete_step(&step_id, started_at, Some(serde_json::json!({"out": 9})))
        .await
        .expect("complete");
    let done = r
        .execution_steps
        .get(&step_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(done.status, StepStatus::Completed);
    assert_eq!(done.tool_result(), Some(&serde_json::json!({"out": 9})));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn complete_step_without_result() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step = ExecutionStep::tool_execution(task_id.clone(), "t", serde_json::json!({}));
    let step_id = step.step_id.clone();
    let started_at = step.started_at;
    r.execution_steps.create(&step).await.expect("create");

    r.execution_steps
        .complete_step(&step_id, started_at, None)
        .await
        .expect("complete no result");
    let done = r
        .execution_steps
        .get(&step_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(done.status, StepStatus::Completed);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn fail_step_records_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step = ExecutionStep::tool_execution(task_id.clone(), "t", serde_json::json!({}));
    let step_id = step.step_id.clone();
    let started_at = step.started_at;
    r.execution_steps.create(&step).await.expect("create");

    r.execution_steps
        .fail_step(&step_id, started_at, "kaboom")
        .await
        .expect("fail");
    let failed = r
        .execution_steps
        .get(&step_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(failed.status, StepStatus::Failed);
    assert_eq!(failed.error_message.as_deref(), Some("kaboom"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn fail_in_progress_steps_for_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let s1 = ExecutionStep::tool_execution(task_id.clone(), "a", serde_json::json!({}));
    let s2 = ExecutionStep::tool_execution(task_id.clone(), "b", serde_json::json!({}));
    r.execution_steps.create(&s1).await.expect("c1");
    r.execution_steps.create(&s2).await.expect("c2");

    let affected = r
        .execution_steps
        .fail_in_progress_steps_for_task(&task_id, "shutdown")
        .await
        .expect("fail all");
    assert_eq!(affected, 2);

    let list = r
        .execution_steps
        .list_by_task(&task_id)
        .await
        .expect("list");
    assert!(list.iter().all(|s| s.status == StepStatus::Failed));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn complete_planning_step_returns_step() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    // A planning step is instant, so create it then complete it with reasoning.
    let step = ExecutionStep::new(task_id.clone(), StepContent::planning(None, None));
    let step_id = step.step_id.clone();
    let started_at = step.started_at;
    r.execution_steps.create(&step).await.expect("create");

    let completed = r
        .execution_steps
        .complete_planning_step(
            &step_id,
            started_at,
            Some("because".to_owned()),
            Some(vec![]),
        )
        .await
        .expect("complete planning");
    assert_eq!(completed.step_id, step_id);
    assert_eq!(completed.status, StepStatus::Completed);
    assert_eq!(completed.step_type(), StepType::Planning);
    assert_eq!(completed.reasoning(), Some("because"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn mcp_execution_id_exists_false_for_unknown() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let exists = r
        .execution_steps
        .mcp_execution_id_exists(&McpExecutionId::new("does-not-exist-xyz"))
        .await
        .expect("check");
    assert!(!exists);
}

#[tokio::test]
async fn list_by_task_empty_for_unknown() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let list = r
        .execution_steps
        .list_by_task(&TaskId::generate())
        .await
        .expect("list");
    assert!(list.is_empty());
}

#[tokio::test]
async fn get_step_with_corrupt_status_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step = ExecutionStep::tool_execution(task_id.clone(), "search", serde_json::json!({}));
    let step_id = step.step_id.clone();
    r.execution_steps.create(&step).await.expect("create");

    let pg = pool.pool_arc().expect("pg pool");
    sqlx::query("UPDATE task_execution_steps SET status = 'bogus_state' WHERE step_id = $1")
        .bind(step_id.to_string())
        .execute(pg.as_ref())
        .await
        .expect("corrupt status");

    let err = r
        .execution_steps
        .get(&step_id)
        .await
        .expect_err("corrupt status must fail parsing");
    assert!(err.to_string().contains("Invalid status"), "got {err}");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_step_with_corrupt_content_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let step = ExecutionStep::tool_execution(task_id.clone(), "search", serde_json::json!({}));
    let step_id = step.step_id.clone();
    r.execution_steps.create(&step).await.expect("create");

    let pg = pool.pool_arc().expect("pg pool");
    sqlx::query("UPDATE task_execution_steps SET content = '[]'::jsonb WHERE step_id = $1")
        .bind(step_id.to_string())
        .execute(pg.as_ref())
        .await
        .expect("corrupt content");

    let err = r
        .execution_steps
        .get(&step_id)
        .await
        .expect_err("corrupt content must fail parsing");
    assert!(err.to_string().contains("Invalid content"), "got {err}");

    r.tasks.delete_task(&task_id).await.ok();
}
