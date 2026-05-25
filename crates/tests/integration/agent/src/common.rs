use anyhow::Result;
use std::env;
use std::sync::Arc;
use systemprompt_agent::repository::task::{RepoCreateTaskParams, TaskRepository};
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::a2a::{Task, TaskState, TaskStatus};
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use uuid::Uuid;

// Serialise this test module against a single in-process gate: each test
// opens its own sqlx pool against Postgres and parallelism easily exhausts
// `max_connections=100` on the shared test DB.
static SERIAL: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn acquire_serial() -> MutexGuard<'static, ()> {
    SERIAL
        .get_or_init(|| async { Mutex::new(()) })
        .await
        .lock()
        .await
}

pub struct Fixture {
    pub pool: sqlx::PgPool,
    pub db: DbPool,
    pub repo: TaskRepository,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub trace_id: TraceId,
    pub context_id: ContextId,
    pub tag: String,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    pub async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url =
            env::var("DATABASE_URL").expect("DATABASE_URL must be set for agent integration tests");
        let db = Database::new_postgres(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();
        let db = Arc::new(db);

        let tag = Uuid::new_v4().simple().to_string();
        let user_id = UserId::new(format!("test_user_{tag}"));
        let session_id = SessionId::new(format!("test_session_{tag}"));
        let trace_id = TraceId::new(format!("test_trace_{tag}"));
        // ContextId is `validated, schema` and requires a UUID v4 string.
        let context_id = ContextId::new(Uuid::new_v4().to_string());

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(user_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("{user_id}@test.invalid"))
            .execute(&pool)
            .await?;

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(context_id.as_str())
            .bind(user_id.as_str())
            .bind(format!("ctx-{tag}"))
            .execute(&pool)
            .await?;

        let repo = TaskRepository::new(&db)?;

        Ok(Self {
            pool,
            db,
            repo,
            user_id,
            session_id,
            trace_id,
            context_id,
            tag,
            _guard: guard,
        })
    }

    pub async fn insert_task(&self, initial: TaskState) -> Result<TaskId> {
        let task_id = TaskId::new(format!("task_{}_{}", self.tag, Uuid::new_v4().simple()));
        let task = Task {
            id: task_id.clone(),
            context_id: self.context_id.clone(),
            status: TaskStatus {
                state: initial,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            history: None,
            artifacts: None,
            metadata: None,
            created_at: Some(chrono::Utc::now()),
            last_modified: Some(chrono::Utc::now()),
        };
        self.repo
            .create_task(RepoCreateTaskParams {
                task: &task,
                user_id: &self.user_id,
                session_id: &self.session_id,
                trace_id: &self.trace_id,
                agent_name: "test-agent",
            })
            .await?;
        Ok(task_id)
    }

    pub async fn current_status(&self, task_id: &TaskId) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT status FROM agent_tasks WHERE task_id = $1")
            .bind(task_id.as_str())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn cleanup(&self) -> Result<()> {
        sqlx::query("DELETE FROM agent_tasks WHERE user_id = $1")
            .bind(self.user_id.as_str())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM user_contexts WHERE context_id = $1")
            .bind(self.context_id.as_str())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(self.user_id.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
