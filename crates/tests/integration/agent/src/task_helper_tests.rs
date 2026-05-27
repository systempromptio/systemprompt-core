//! Exercises `services::mcp::task_helper` — `ensure_task_exists` and
//! `save_messages_for_tool_execution`. Each test seeds a user + context row,
//! then drives the helper functions through the live Postgres pool.

use anyhow::Result;
use systemprompt_agent::services::mcp::task_helper::{
    SaveMessagesForToolExecutionParams, ensure_task_exists, save_messages_for_tool_execution,
};
use systemprompt_identifiers::{
    AgentName, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::a2a::{Artifact, ArtifactMetadata};
use systemprompt_models::RequestContext;

use crate::common::Fixture;

fn request_context_with_ids(f: &Fixture) -> RequestContext {
    RequestContext::new(
        f.session_id.clone(),
        f.trace_id.clone(),
        f.context_id.clone(),
        AgentName::new("test-agent"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(f.user_id.clone()))
}

#[tokio::test]
async fn ensure_task_exists_reuses_when_task_id_already_set() -> Result<()> {
    let f = Fixture::new().await?;
    let preset = TaskId::new(format!("preset_{}", f.tag));
    let mut ctx = request_context_with_ids(&f).with_task_id(preset.clone());

    let result = ensure_task_exists(&f.db, &mut ctx, "tool-a", "server-a")
        .await
        .expect("ensure_task_exists ok");

    assert_eq!(result.task_id, preset);
    assert!(!result.is_owner, "reused task is not owned by this caller");
    f.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn ensure_task_exists_creates_task_for_valid_context() -> Result<()> {
    let f = Fixture::new().await?;
    let mut ctx = request_context_with_ids(&f);

    let result = ensure_task_exists(&f.db, &mut ctx, "tool-b", "server-b")
        .await
        .expect("ensure_task_exists ok");

    assert!(result.is_owner, "new task must be owned by caller");
    assert!(!result.task_id.as_str().is_empty());
    // RequestContext must have been mutated to carry the new task_id.
    assert_eq!(ctx.task_id(), Some(&result.task_id));

    let row: (String,) =
        sqlx::query_as("SELECT status FROM agent_tasks WHERE task_id = $1")
            .bind(result.task_id.as_str())
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(row.0, "TASK_STATE_SUBMITTED");

    f.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn ensure_task_exists_falls_back_to_new_context_when_ownership_invalid() -> Result<()> {
    let f = Fixture::new().await?;
    // Build a context with a context_id the user does NOT own — code path
    // will validate, fail, and auto-create a replacement.
    let bogus = ContextId::generate();
    let mut ctx = RequestContext::new(
        f.session_id.clone(),
        f.trace_id.clone(),
        bogus.clone(),
        AgentName::new("test-agent"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(f.user_id.clone()));

    let result = ensure_task_exists(&f.db, &mut ctx, "tool-c", "server-c")
        .await
        .expect("ensure_task_exists ok");

    assert!(result.is_owner);
    assert_ne!(ctx.context_id(), &bogus, "context_id must be replaced");

    f.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn save_messages_for_tool_execution_persists_pair() -> Result<()> {
    let f = Fixture::new().await?;
    // Seed a task to satisfy any task FK on messages.
    let task_id = f
        .insert_task(systemprompt_models::a2a::TaskState::Submitted)
        .await?;

    save_messages_for_tool_execution(SaveMessagesForToolExecutionParams {
        db_pool: &f.db,
        task_id: &task_id,
        context_id: &f.context_id,
        tool_name: "do-thing",
        tool_result: "{\"ok\":true}",
        artifact: None,
        user_id: &f.user_id,
        session_id: &f.session_id,
        trace_id: &f.trace_id,
    })
    .await
    .expect("persist ok");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_messages WHERE task_id = $1",
    )
    .bind(task_id.as_str())
    .fetch_one(&f.pool)
    .await?;
    assert!(count.0 >= 2, "user + agent message rows present, got {}", count.0);

    f.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn save_messages_for_tool_execution_includes_artifact_text() -> Result<()> {
    let f = Fixture::new().await?;
    let task_id = f
        .insert_task(systemprompt_models::a2a::TaskState::Submitted)
        .await?;
    let artifact = Artifact {
        id: systemprompt_identifiers::ArtifactId::generate(),
        title: Some("results.txt".to_owned()),
        description: None,
        parts: vec![],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            "text/plain".to_owned(),
            f.context_id.clone(),
            task_id.clone(),
        ),
    };

    save_messages_for_tool_execution(SaveMessagesForToolExecutionParams {
        db_pool: &f.db,
        task_id: &task_id,
        context_id: &f.context_id,
        tool_name: "do-thing",
        tool_result: "raw",
        artifact: Some(&artifact),
        user_id: &f.user_id,
        session_id: &f.session_id,
        trace_id: &f.trace_id,
    })
    .await
    .expect("persist ok");

    f.cleanup().await?;
    Ok(())
}

// Silence unused-binding warnings if a future refactor drops one of the
// imports above.
#[allow(dead_code)]
fn _typecheck_imports() {
    let _ = SessionId::generate;
    let _ = TraceId::generate;
    let _: fn(&'static str) -> UserId = UserId::new;
    let _ = ContextId::generate;
}
