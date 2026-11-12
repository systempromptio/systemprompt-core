use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::{RepoCreateTaskParams, TaskRepository};
use systemprompt_agent::repository::{ContextRepository, RuntimeTask};
use systemprompt_agent::services::context::ContextService;
use systemprompt_agent::services::context_provider::ContextProviderService;
use systemprompt_agent::services::execution_tracking::ExecutionTrackingService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::PlannedTool;
use systemprompt_models::a2a::{Task, TaskState, TaskStatus};
use systemprompt_test_fixtures::agent_reads::StepReadsExt;
use systemprompt_test_fixtures::test_db_pool;
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
        let db = test_db_pool().await;
        let pool = db.pool().as_ref().clone();

        let tag = Uuid::new_v4().simple().to_string();
        let user_id = UserId::new(format!("svc_user_{tag}"));
        let session_id = SessionId::new(format!("svc_session_{tag}"));
        let trace_id = TraceId::new(format!("svc_trace_{tag}"));
        let context_id = ContextId::try_new(Uuid::new_v4().to_string()).expect("valid ContextId");

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
        Ok(self.insert_runtime_task().await?.task_id().clone())
    }

    async fn insert_runtime_task(&self) -> Result<RuntimeTask> {
        let repos = systemprompt_test_fixtures::a2a_repositories(&self.db);
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
        let runtime = repos
            .tasks
            .create_task(RepoCreateTaskParams {
                task: &task,
                user_id: &self.user_id,
                session_id: &self.session_id,
                trace_id: &self.trace_id,
                agent_name: &AgentName::new("svc-agent"),
            })
            .await?;
        Ok(runtime)
    }

    async fn save_turn(
        &self,
        runtime: &RuntimeTask,
        user_text: &str,
        agent_text: &str,
    ) -> Result<()> {
        use systemprompt_agent::models::a2a::{Message, MessageRole, Part};
        use systemprompt_agent::repository::task::UpdateTaskAndSaveMessagesParams;
        use systemprompt_identifiers::MessageId;

        let message = |role: MessageRole, text: &str| Message {
            role,
            message_id: MessageId::new(format!("h_{}_{}", self.tag, Uuid::new_v4().simple())),
            task_id: Some(runtime.task_id().clone()),
            context_id: runtime.context_id().clone(),
            parts: vec![Part::text(text)],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };
        let task = Task {
            id: runtime.task_id().clone(),
            context_id: runtime.context_id().clone(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
                timestamp: Some(Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: None,
            created_at: Some(Utc::now()),
            last_modified: Some(Utc::now()),
        };
        systemprompt_test_fixtures::a2a_repositories(&self.db)
            .tasks
            .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
                runtime,
                task: &task,
                user_message: &message(MessageRole::User, user_text),
                agent_message: &message(MessageRole::Agent, agent_text),
                user_id: Some(&self.user_id),
                session_id: &self.session_id,
                trace_id: &self.trace_id,
            })
            .await?;
        Ok(())
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
    let exec_repo = Arc::new(ExecutionStepRepository::new(&fx.db));
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
        .track_tool_execution(task_id.clone(), "echo", serde_json::json!({"x": 1}))
        .await?;
    svc.complete(tracked_tool, Some(serde_json::json!({"ok": true})))
        .await?;

    let _completion = svc.track_completion(task_id.clone()).await?;

    let steps = ExecutionStepRepository::new(&fx.db)
        .steps_for_task(
            &TaskRepository::new(&fx.db, crate::common::session_usage(&fx.db)?),
            &task_id,
        )
        .await?;
    assert!(steps.len() >= 4);

    let got = svc.find_step(&tool_step.step_id).await?;
    assert!(got.is_some());

    let count = svc.fail_in_progress_steps(&task_id, "agent halted").await?;
    let _ = count;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_service_load_history_for_empty_context_returns_empty() -> Result<()> {
    let fx = ServicesFixture::new().await?;
    let svc = ContextService::new(
        TaskRepository::new(&fx.db, crate::common::session_usage(&fx.db)?),
        ContextRepository::new(&fx.db),
    );
    let history = svc
        .load_conversation_history(&fx.context_id, &fx.user_id)
        .await?;
    assert!(history.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_provider_service_lists_user_contexts() -> Result<()> {
    let fx = ServicesFixture::new().await?;
    let svc = ContextProviderService::new(ContextRepository::new(&fx.db));
    let listed = svc.list_contexts_with_stats(&fx.user_id).await?;
    assert!(listed.iter().any(|c| c.context_id == fx.context_id));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_provider_service_get_context_returns_data() -> Result<()> {
    use systemprompt_traits::ContextProvider;
    let fx = ServicesFixture::new().await?;
    let svc = ContextProviderService::new(ContextRepository::new(&fx.db));
    let ctx = svc.get_context(&fx.context_id, &fx.user_id).await?;
    assert_eq!(ctx.context_id, fx.context_id);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn execution_tracking_service_list_steps_for_unknown_task() -> Result<()> {
    use systemprompt_identifiers::TaskId;
    let fx = ServicesFixture::new().await?;
    let steps = ExecutionStepRepository::new(&fx.db)
        .steps_for_task(
            &TaskRepository::new(&fx.db, crate::common::session_usage(&fx.db)?),
            &TaskId::new("nonexistent-task-id"),
        )
        .await?;
    assert!(steps.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_service_loads_ordered_user_and_agent_history_across_tasks() -> Result<()> {
    use systemprompt_agent::services::context::ContextService;

    let fx = ServicesFixture::new().await?;
    let first_task = fx.insert_runtime_task().await?;
    let second_task = fx.insert_runtime_task().await?;
    fx.save_turn(&first_task, "first question", "first answer")
        .await?;
    fx.save_turn(&second_task, "follow-up question", "follow-up answer")
        .await?;

    let context_svc = ContextService::new(
        TaskRepository::new(&fx.db, crate::common::session_usage(&fx.db)?),
        ContextRepository::new(&fx.db),
    );
    let history = context_svc
        .load_conversation_history(&fx.context_id, &fx.user_id)
        .await?;
    let turns: Vec<_> = history
        .iter()
        .map(|message| (message.role.clone(), message.content.clone()))
        .collect();
    assert_eq!(
        turns,
        vec![
            (
                systemprompt_models::MessageRole::User,
                "first question".to_owned()
            ),
            (
                systemprompt_models::MessageRole::Assistant,
                "first answer".to_owned()
            ),
            (
                systemprompt_models::MessageRole::User,
                "follow-up question".to_owned()
            ),
            (
                systemprompt_models::MessageRole::Assistant,
                "follow-up answer".to_owned()
            ),
        ]
    );

    fx.cleanup().await?;
    Ok(())
}
