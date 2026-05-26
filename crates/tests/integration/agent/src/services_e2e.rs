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

#[tokio::test]
async fn message_service_persists_messages_for_task() -> Result<()> {
    use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
    use systemprompt_agent::services::message::{MessageService, PersistMessagesParams};
    use systemprompt_identifiers::MessageId;

    let fx = ServicesFixture::new().await?;
    let svc = MessageService::new(&fx.db)?;

    let task_id = fx.insert_task().await?;
    let messages = vec![
        Message {
            role: MessageRole::User,
            message_id: MessageId::new(format!("m1_{}", fx.tag)),
            task_id: Some(task_id.clone()),
            context_id: fx.context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: "first user message".into(),
            })],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
        Message {
            role: MessageRole::Agent,
            message_id: MessageId::new(format!("m2_{}", fx.tag)),
            task_id: Some(task_id.clone()),
            context_id: fx.context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: "agent reply".into(),
            })],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
    ];

    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    assert_eq!(seqs.len(), 2);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_service_persist_empty_list_returns_empty() -> Result<()> {
    use systemprompt_agent::services::message::{MessageService, PersistMessagesParams};

    let fx = ServicesFixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task().await?;

    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: Vec::new(),
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    assert!(seqs.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_service_creates_tool_execution_message() -> Result<()> {
    use systemprompt_agent::services::message::{
        CreateToolExecutionMessageParams, MessageService,
    };
    use systemprompt_models::RequestContext;

    let fx = ServicesFixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task().await?;

    use systemprompt_identifiers::AgentName;
    let request_context = RequestContext::new(
        fx.session_id.clone(),
        fx.trace_id.clone(),
        fx.context_id.clone(),
        AgentName::new("svc-agent"),
    );

    let (msg_id, seq) = svc
        .create_tool_execution_message(CreateToolExecutionMessageParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            tool_name: "echo",
            tool_args: &serde_json::json!({"x": 1}),
            request_context: &request_context,
        })
        .await?;
    assert!(!msg_id.is_empty());
    assert!(seq >= 0);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_service_loads_history_with_messages() -> Result<()> {
    use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
    use systemprompt_agent::services::context::ContextService;
    use systemprompt_agent::services::message::{MessageService, PersistMessagesParams};
    use systemprompt_identifiers::MessageId;

    let fx = ServicesFixture::new().await?;
    let task_id = fx.insert_task().await?;
    let msg_svc = MessageService::new(&fx.db)?;
    let context_svc = ContextService::new(&fx.db)?;

    msg_svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: vec![Message {
                role: MessageRole::User,
                message_id: MessageId::new(format!("h1_{}", fx.tag)),
                task_id: Some(task_id.clone()),
                context_id: fx.context_id.clone(),
                parts: vec![Part::Text(TextPart {
                    text: "history user msg".into(),
                })],
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            }],
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;

    let _history = context_svc.load_conversation_history(&fx.context_id).await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_provider_service_get_context_returns_data() -> Result<()> {
    use systemprompt_traits::ContextProvider;
    let fx = ServicesFixture::new().await?;
    let svc = ContextProviderService::new(&fx.db)?;
    let ctx = svc.get_context(&fx.context_id, &fx.user_id).await?;
    assert_eq!(ctx.context_id, fx.context_id);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn execution_tracking_service_list_steps_for_unknown_task() -> Result<()> {
    use systemprompt_identifiers::TaskId;
    let fx = ServicesFixture::new().await?;
    let exec_repo = Arc::new(ExecutionStepRepository::new(&fx.db)?);
    let svc = ExecutionTrackingService::new(exec_repo);

    let steps = svc
        .get_steps_by_task(&TaskId::new("nonexistent-task-id"))
        .await?;
    assert!(steps.is_empty());

    fx.cleanup().await?;
    Ok(())
}
