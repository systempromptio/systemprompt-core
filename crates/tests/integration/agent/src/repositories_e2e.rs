use anyhow::Result;
use chrono::Utc;
use systemprompt_agent::models::a2a::protocol::PushNotificationConfig;
use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Message, MessageRole, Part,
    TextPart,
};
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::repository::content::artifact::ArtifactRepository;
use systemprompt_agent::repository::context::message::{
    MessageRepository, PersistMessageSqlxParams,
};
use systemprompt_agent::repository::context::{ContextNotificationRepository, ContextRepository};
use systemprompt_agent::repository::task::{
    RepoCreateTaskParams, TaskConstructor, TaskRepository, UpdateTaskAndSaveMessagesParams,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AgentId, ArtifactId, ConfigId, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::a2a::{Task, TaskState, TaskStatus};
use systemprompt_models::{ExecutionStep, StepContent};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
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

struct E2EFixture {
    db: DbPool,
    pool: sqlx::PgPool,
    user_id: UserId,
    session_id: SessionId,
    trace_id: TraceId,
    context_id: ContextId,
    tag: String,
    _guard: MutexGuard<'static, ()>,
}

impl E2EFixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = fixture_database_url()?;
        let db = fixture_db_pool(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();

        let tag = Uuid::new_v4().simple().to_string();
        let user_id = UserId::new(format!("e2e_user_{tag}"));
        let session_id = SessionId::new(format!("e2e_session_{tag}"));
        let trace_id = TraceId::new(format!("e2e_trace_{tag}"));
        let context_id = ContextId::new(Uuid::new_v4().to_string());

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(user_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("{user_id}@e2e.invalid"))
            .execute(&pool)
            .await?;

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(context_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("e2e-ctx-{tag}"))
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

    async fn insert_task(&self, repo: &TaskRepository, state: TaskState) -> Result<TaskId> {
        let task_id = TaskId::new(format!("e2e_task_{}_{}", self.tag, Uuid::new_v4().simple()));
        let task = Task {
            id: task_id.clone(),
            context_id: self.context_id.clone(),
            status: TaskStatus {
                state,
                message: None,
                timestamp: Some(Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: None,
            created_at: Some(Utc::now()),
            last_modified: Some(Utc::now()),
        };
        repo.create_task(RepoCreateTaskParams {
            task: &task,
            user_id: &self.user_id,
            session_id: &self.session_id,
            trace_id: &self.trace_id,
            agent_name: "e2e-agent",
        })
        .await?;
        Ok(task_id)
    }

    async fn cleanup(&self) -> Result<()> {
        // task_artifacts / task_messages cascade via FK; remove agent_tasks first.
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
async fn a2a_repositories_construct_and_share_pool() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    // Pool sanity: agent_services repo can query an empty result.
    let running = repos.agent_services.list_running_agents().await?;
    assert!(running.iter().all(|r| !r.name.is_empty()));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn task_repository_create_get_list_round_trip() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;

    let t1 = fx.insert_task(&repos.tasks, TaskState::Submitted).await?;
    let t2 = fx.insert_task(&repos.tasks, TaskState::Submitted).await?;

    let got = repos.tasks.get_task(&t1).await?;
    assert!(got.is_some(), "task should be retrievable");

    let by_ctx = repos.tasks.list_tasks_by_context(&fx.context_id).await?;
    assert!(by_ctx.len() >= 2);

    let by_user = repos
        .tasks
        .get_tasks_by_user_id(&fx.user_id, Some(10), Some(0))
        .await?;
    assert!(by_user.iter().any(|t| t.id == t1));
    assert!(by_user.iter().any(|t| t.id == t2));

    repos
        .tasks
        .track_agent_in_context(&fx.context_id, "e2e-agent")
        .await?;

    let now = Utc::now();
    repos
        .tasks
        .update_task_state(&t1, TaskState::Working, &now)
        .await?;
    let after = repos.tasks.get_task(&t1).await?.unwrap();
    assert!(matches!(after.status.state, TaskState::Working));

    repos
        .tasks
        .apply_notification_status(&t1, "completed", &now)
        .await?;
    repos
        .tasks
        .update_task_failed_with_error(&t2, "boom", &now)
        .await?;
    let ctx_info = repos.tasks.get_task_context_info(&t1).await?;
    assert!(ctx_info.is_some());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_persists_all_part_kinds_and_reads_back() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let messages = MessageRepository::new(&fx.db)?;

    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    let message_id = MessageId::new(format!("msg_{}", Uuid::new_v4().simple()));
    let mut data_obj = serde_json::Map::new();
    data_obj.insert("k".into(), serde_json::json!("v"));
    let message = Message {
        role: MessageRole::User,
        parts: vec![
            Part::Text(TextPart {
                text: "hello".into(),
            }),
            Part::Data(DataPart { data: data_obj }),
            Part::File(FilePart {
                file: FileContent {
                    name: Some("note.txt".into()),
                    mime_type: Some("text/plain".into()),
                    bytes: Some("aGVsbG8=".into()),
                    url: None,
                },
            }),
        ],
        message_id: message_id.clone(),
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        metadata: Some(serde_json::json!({"clientMessageId": "cm-1"})),
        extensions: None,
        reference_task_ids: None,
    };

    let mut tx = fx.pool.begin().await?;
    messages
        .persist_message_sqlx(PersistMessageSqlxParams {
            tx: &mut tx,
            message: &message,
            task_id: &task_id,
            context_id: &fx.context_id,
            sequence_number: 0,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    tx.commit().await?;

    assert!(messages.get_next_sequence_number(&task_id).await? >= 1);

    let read_back_by_task = messages.get_messages_by_task(&task_id).await?;
    assert_eq!(read_back_by_task.len(), 1);
    assert_eq!(read_back_by_task[0].parts.len(), 3);

    let read_back_by_ctx = messages.get_messages_by_context(&fx.context_id).await?;
    assert!(!read_back_by_ctx.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn artifact_repository_create_and_query_paths() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let artifacts = ArtifactRepository::new(&fx.db)?;

    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    let artifact_id = ArtifactId::new(format!("art_{}", Uuid::new_v4().simple()));
    let metadata = ArtifactMetadata::new("text".to_owned(), fx.context_id.clone(), task_id.clone())
        .with_source("mcp_tool".to_owned())
        .with_tool_name("echo".to_owned())
        .with_fingerprint("fp-1".to_owned());

    let artifact = Artifact {
        id: artifact_id.clone(),
        title: Some("E2E Artifact".into()),
        description: Some("desc".into()),
        parts: vec![Part::Text(TextPart {
            text: "artifact body".into(),
        })],
        extensions: vec![],
        metadata,
    };

    artifacts
        .create_artifact(&task_id, &fx.context_id, &artifact)
        .await?;

    let by_task = artifacts.get_artifacts_by_task(&task_id).await?;
    assert_eq!(by_task.len(), 1);
    assert_eq!(by_task[0].id, artifact_id);

    let by_ctx = artifacts.get_artifacts_by_context(&fx.context_id).await?;
    assert!(!by_ctx.is_empty());

    let by_user = artifacts
        .get_artifacts_by_user_id(&fx.user_id, Some(50))
        .await?;
    assert!(by_user.iter().any(|a| a.id == artifact_id));

    let by_id = artifacts.get_artifact_by_id(&artifact_id).await?;
    assert!(by_id.is_some());

    let all = artifacts.get_all_artifacts(Some(10)).await?;
    assert!(all.iter().any(|a| a.id == artifact_id));

    artifacts.delete_artifact(&artifact_id).await?;
    let after_delete = artifacts.get_artifact_by_id(&artifact_id).await?;
    assert!(after_delete.is_none());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn push_notification_config_round_trip() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    let cfg = PushNotificationConfig {
        endpoint: "https://example.invalid/webhook".to_owned(),
        headers: Some({
            let mut m = serde_json::Map::new();
            m.insert("X-Test".into(), serde_json::json!("yes"));
            m
        }),
        url: "https://example.invalid/webhook".to_owned(),
        token: Some("tok".to_owned()),
        authentication: None,
    };

    let config_id_str = repos
        .push_notification_configs
        .add_config(&task_id, &cfg)
        .await?;

    let config_id = ConfigId::new(config_id_str);
    let read = repos
        .push_notification_configs
        .get_config(&task_id, &config_id)
        .await?;
    assert!(read.is_some());
    let read = read.unwrap();
    assert_eq!(read.url, cfg.url);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn execution_step_repository_lifecycle() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    let plan_step = ExecutionStep::new(
        task_id.clone(),
        StepContent::planning(Some("plan".into()), None),
    );
    let tool_step = ExecutionStep::new(
        task_id.clone(),
        StepContent::ToolExecution {
            tool_name: "echo".into(),
            tool_arguments: serde_json::json!({"x": 1}),
            tool_result: None,
        },
    );

    repos.execution_steps.create(&plan_step).await?;
    repos.execution_steps.create(&tool_step).await?;

    let by_task = repos.execution_steps.list_by_task(&task_id).await?;
    assert_eq!(by_task.len(), 2);

    let got = repos.execution_steps.get(&plan_step.step_id).await?;
    assert!(got.is_some());

    repos
        .execution_steps
        .complete_step(
            &tool_step.step_id,
            tool_step.started_at,
            Some(serde_json::json!({"ok": true})),
        )
        .await?;

    repos
        .execution_steps
        .fail_step(&plan_step.step_id, plan_step.started_at, "planner died")
        .await?;

    assert!(
        !repos
            .execution_steps
            .mcp_execution_id_exists(&systemprompt_identifiers::McpExecutionId::new(
                "non-existent",
            ))
            .await?
    );

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_repository_crud_and_listing() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let ctx_repo = ContextRepository::new(&fx.db)?;

    let new_ctx = ctx_repo
        .create_context(&fx.user_id, None, "secondary-ctx", ContextKind::User)
        .await?;

    ctx_repo
        .validate_context_ownership(&new_ctx, &fx.user_id)
        .await?;

    ctx_repo
        .update_context_name(&new_ctx, &fx.user_id, "renamed-ctx")
        .await?;

    let _ = ctx_repo.find_by_session_id(&fx.session_id).await?;

    let user_for_ctx = ctx_repo.find_user_id_for_context(&new_ctx).await?;
    assert_eq!(user_for_ctx.as_ref(), Some(&fx.user_id));

    let fetched = ctx_repo.get_context(&new_ctx, &fx.user_id).await?;
    assert_eq!(fetched.context_id, new_ctx);

    let basic = ctx_repo.list_contexts_basic(&fx.user_id).await?;
    assert!(!basic.is_empty());

    let with_stats = ctx_repo.list_contexts_with_stats(&fx.user_id).await?;
    assert!(!with_stats.is_empty());

    let since = Utc::now() - chrono::Duration::hours(1);
    let _events = ctx_repo.get_context_events_since(&new_ctx, since).await?;

    ctx_repo.delete_context(&new_ctx, &fx.user_id).await?;
    assert!(ctx_repo.get_context(&new_ctx, &fx.user_id).await.is_err());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_notification_repository_insert_and_broadcast() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let notif = ContextNotificationRepository::new(&fx.db)?;

    let agent_id = AgentId::new("e2e-agent");
    let id = notif
        .insert(
            &fx.context_id,
            &agent_id,
            "notifications/messageAdded",
            &serde_json::json!({"msg": "hi"}),
        )
        .await?;
    notif.mark_broadcasted(id).await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_service_repository_register_status_cycle() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let name = format!("e2e-svc-{}", fx.tag);

    repos
        .agent_services
        .register_agent_starting(&name, 99999, 19999)
        .await?;
    repos.agent_services.mark_running(&name).await?;

    let status = repos.agent_services.get_agent_status(&name).await?;
    assert!(status.is_some());
    assert_eq!(status.unwrap().status, "running");

    repos
        .agent_services
        .update_health_status(&name, "running")
        .await?;
    let running = repos.agent_services.list_running_agents().await?;
    assert!(running.iter().any(|r| r.name == name));
    let running_pids = repos.agent_services.list_running_agent_pids().await?;
    assert!(running_pids.iter().any(|r| r.name == name));

    repos.agent_services.mark_crashed(&name).await?;
    repos.agent_services.mark_error(&name).await?;
    repos.agent_services.mark_stopped(&name).await?;
    repos
        .agent_services
        .register_agent(&name, 12345, 20000)
        .await?;
    repos.agent_services.remove_agent_service(&name).await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn task_constructor_assembles_task_with_messages_and_artifacts() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let messages = MessageRepository::new(&fx.db)?;
    let artifacts = ArtifactRepository::new(&fx.db)?;

    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    // Insert message with text + data parts
    let message_id = MessageId::new(format!("ctor_msg_{}", Uuid::new_v4().simple()));
    let mut data_obj = serde_json::Map::new();
    data_obj.insert("k".into(), serde_json::json!(42));
    let message = Message {
        role: MessageRole::Agent,
        parts: vec![
            Part::Text(TextPart {
                text: "agent reply".into(),
            }),
            Part::Data(DataPart { data: data_obj }),
        ],
        message_id,
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: Some(vec![task_id.clone()]),
    };

    let mut tx = fx.pool.begin().await?;
    messages
        .persist_message_sqlx(PersistMessageSqlxParams {
            tx: &mut tx,
            message: &message,
            task_id: &task_id,
            context_id: &fx.context_id,
            sequence_number: 0,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    tx.commit().await?;

    // Artifact with text + file parts
    let artifact_id = ArtifactId::new(format!("art_ctor_{}", Uuid::new_v4().simple()));
    let metadata = ArtifactMetadata::new("text".to_owned(), fx.context_id.clone(), task_id.clone());
    let artifact = Artifact {
        id: artifact_id,
        title: Some("ctor artifact".into()),
        description: None,
        parts: vec![
            Part::Text(TextPart {
                text: "result text".into(),
            }),
            Part::File(FilePart {
                file: FileContent {
                    name: Some("a.bin".into()),
                    mime_type: Some("application/octet-stream".into()),
                    bytes: Some("aGVsbG8=".into()),
                    url: None,
                },
            }),
        ],
        extensions: vec![],
        metadata,
    };
    artifacts
        .create_artifact(&task_id, &fx.context_id, &artifact)
        .await?;

    // Now construct + verify
    let ctor = TaskConstructor::new(&fx.db)?;
    let single = ctor.construct_task_from_task_id(&task_id).await?;
    assert_eq!(single.id, task_id);
    assert!(single.history.is_some());
    assert!(single.artifacts.is_some());

    let batch = ctor
        .construct_tasks_batch(std::slice::from_ref(&task_id))
        .await?;
    assert_eq!(batch.len(), 1);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn task_repository_update_task_and_save_messages_and_delete() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;
    let task_id = fx.insert_task(&repos.tasks, TaskState::Submitted).await?;

    let user_msg = Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "user input".into(),
        })],
        message_id: MessageId::new(format!("um_{}", Uuid::new_v4().simple())),
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    let agent_msg = Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart {
            text: "agent output".into(),
        })],
        message_id: MessageId::new(format!("am_{}", Uuid::new_v4().simple())),
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    let task = Task {
        id: task_id.clone(),
        context_id: fx.context_id.clone(),
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

    repos
        .tasks
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;

    repos.tasks.delete_task(&task_id).await?;
    assert!(repos.tasks.get_task(&task_id).await?.is_none());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn task_and_message_writes_increment_session_counters() -> Result<()> {
    let fx = E2EFixture::new().await?;
    let repos = A2ARepositories::new(&fx.db)?;

    sqlx::query("INSERT INTO user_sessions (session_id, user_id) VALUES ($1, $2)")
        .bind(fx.session_id.as_str())
        .bind(fx.user_id.as_str())
        .execute(&fx.pool)
        .await?;

    let task_id = fx.insert_task(&repos.tasks, TaskState::Working).await?;

    let make_message = |role: MessageRole, text: &str| Message {
        role,
        parts: vec![Part::Text(TextPart { text: text.into() })],
        message_id: MessageId::new(format!("cnt_msg_{}", Uuid::new_v4().simple())),
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };
    let task = repos.tasks.get_task(&task_id).await?.expect("task present");
    repos
        .tasks
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task: &task,
            user_message: &make_message(MessageRole::User, "hi"),
            agent_message: &make_message(MessageRole::Agent, "hello"),
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;

    let (task_count, message_count): (i32, i32) =
        sqlx::query_as("SELECT task_count, message_count FROM user_sessions WHERE session_id = $1")
            .bind(fx.session_id.as_str())
            .fetch_one(&fx.pool)
            .await?;
    assert_eq!(task_count, 1, "create_task must increment task_count");
    assert_eq!(
        message_count, 2,
        "update_task_and_save_messages must count both messages"
    );

    sqlx::query("DELETE FROM user_sessions WHERE session_id = $1")
        .bind(fx.session_id.as_str())
        .execute(&fx.pool)
        .await?;
    fx.cleanup().await?;
    Ok(())
}
