use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::RepoCreateTaskParams;
use systemprompt_agent::services::context::ContextService;
use systemprompt_agent::services::context_provider::ContextProviderService;
use systemprompt_agent::services::execution_tracking::ExecutionTrackingService;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::a2a::{Task, TaskState, TaskStatus};
use systemprompt_models::PlannedTool;
use systemprompt_traits::ContextProvider;
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use uuid::Uuid;

static SERIAL: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn acquire_serial() -> MutexGuard<'static, ()> {
    SERIAL
        .get_or_init(|| async { Mutex::new(()) })
        .await
        .lock()
        .await
}

struct ServicesFixture {
    db: DbPool,
    pool: sqlx::PgPool,
    user_id: UserId,
    session_id: SessionId,
    trace_id: TraceId,
    context_id: ContextId,
    tag: String,
    _guard: MutexGuard<'static, ()>,
}

impl ServicesFixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for agent integration tests");
        let database = Database::new_postgres(&url).await?;
        let pool = database.pool_arc()?.as_ref().clone();
        let db: DbPool = Arc::new(database);

        let tag = Uuid::new_v4().simple().to_string();
        let user_id = UserId::new(format!("svc_user_{tag}"));
        let session_id = SessionId::new(format!("svc_session_{tag}"));
        let trace_id = TraceId::new(format!("svc_trace_{tag}"));
        let context_id = ContextId::new(Uuid::new_v4().to_string());

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(user_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("{user_id}@svc.invalid"))
            .execute(&pool)
            .await?;

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(context_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("svc-ctx-{tag}"))
            .execute(&pool)
            .await?;

        Ok(Self {
            db,
            pool,
            user_id,
            session_id,
            trace_id,
            context_id,
            tag,
            _guard: guard,
        })
    }

    async fn insert_task(&self) -> Result<TaskId> {
        let repos = A2ARepositories::new(&self.db)?;
        let task_id = TaskId::new(format!("svc_task_{}_{}", self.tag, Uuid::new_v4().simple()));
        let task = Task {
            id: task_id.clone(),
            context_id: self.context_id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: None,
            created_at: Some(Utc::now()),
            last_modified: Some(Utc::now()),
        };
        repos
            .tasks
            .create_task(RepoCreateTaskParams {
                task: &task,
                user_id: &self.user_id,
                session_id: &self.session_id,
                trace_id: &self.trace_id,
                agent_name: "svc-agent",
            })
            .await?;
        Ok(task_id)
    }

    async fn cleanup(&self) -> Result<()> {
        sqlx::query("DELETE FROM agent_tasks WHERE user_id = $1")
            .bind(self.user_id.as_str())
            .execute(&self.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(self.user_id.as_str())
            .execute(&self.pool)
            .await
            .ok();
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(self.user_id.as_str())
            .execute(&self.pool)
            .await
            .ok();
        Ok(())
    }

}

#[tokio::test]
async fn execution_tracking_service_full_lifecycle() -> Result<()> {
    let fx = ServicesFixture::new().await?;
    let exec_repo = Arc::new(ExecutionStepRepository::new(&fx.db)?);
    let svc = ExecutionTrackingService::new(exec_repo);

    let task_id = fx.insert_task().await?;

    let _understanding = svc.track_understanding(task_id.clone()).await?;

    let (tracked_plan, _plan_step) = svc
        .track_planning_async(
            task_id.clone(),
            Some("plan reasoning".into()),
            Some(vec![PlannedTool {
                tool_name: "echo".into(),
                arguments: serde_json::json!({"x": 1}),
            }]),
        )
        .await?;
    let _completed_plan = svc
        .complete_planning(tracked_plan, Some("done".into()), None)
        .await?;

    let (tracked_tool, tool_step) = svc
        .track_tool_execution(
            task_id.clone(),
            "echo",
            serde_json::json!({"x": 1}),
        )
        .await?;
    svc.complete(tracked_tool, Some(serde_json::json!({"ok": true}))).await?;

    let _completion = svc.track_completion(task_id.clone()).await?;

    let steps = svc.get_steps_by_task(&task_id).await?;
    assert!(steps.len() >= 4);

    let got = svc.get_step(&tool_step.step_id).await?;
    assert!(got.is_some());

    let count = svc.fail_in_progress_steps(&task_id, "agent halted").await?;
    let _ = count;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_service_load_history_for_empty_context_returns_empty() -> Result<()> {
    let fx = ServicesFixture::new().await?;
    let svc = ContextService::new(&fx.db)?;
    let history = svc.load_conversation_history(&fx.context_id).await?;
    assert!(history.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_provider_service_lists_user_contexts() -> Result<()> {
    let fx = ServicesFixture::new().await?;
    let svc = ContextProviderService::new(&fx.db)?;
    let listed = svc.list_contexts_with_stats(&fx.user_id).await?;
    assert!(listed.iter().any(|c| c.context_id == fx.context_id));
    fx.cleanup().await?;
    Ok(())
}
