// DB-backed tests for the agent repository layer. Each module covers one
// sub-repository (agent_service, context, message, task, artifact, execution,
// push_notification) plus the aggregate `A2ARepositories` facade.
//
// Every test early-returns when DATABASE_URL is unset so the suite still
// compiles and passes in environments without a migrated Postgres.

mod agent_service;
mod aggregate;
mod artifact;
mod artifact_parts;
mod batch_builders;
mod context;
mod context_notifications;
mod execution;
mod message;
mod message_tx;
mod push_notification;
mod task;

use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};

// Returns a live pool, or None when no test database is configured.
pub(crate) async fn try_pool() -> Option<DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

pub(crate) fn repos(pool: &DbPool) -> A2ARepositories {
    A2ARepositories::new(pool).expect("repositories")
}

// Seeds a user + session and returns their ids.
pub(crate) async fn seed_user_and_session(pool: &DbPool) -> (UserId, SessionId) {
    let user_id = systemprompt_test_fixtures::unique_user_id("agentrepo");
    let session_id = SessionId::generate();
    let email = format!("{}@agentrepo.invalid", user_id.as_str());
    systemprompt_test_fixtures::seed_user_row(pool, &user_id, &email)
        .await
        .expect("seed user");
    systemprompt_test_fixtures::seed_user_session(pool, &user_id, &session_id)
        .await
        .expect("seed session");
    (user_id, session_id)
}

// Creates a context owned by the user and a submitted task within it; returns
// (context_id, task_id). Exercises the context + task create paths.
pub(crate) async fn seed_context_and_task(
    repos: &A2ARepositories,
    user_id: &UserId,
    session_id: &SessionId,
) -> (ContextId, TaskId) {
    let ctx_repo = systemprompt_agent::repository::ContextRepository::new(repos.db_pool())
        .expect("context repo");
    let context_id = ctx_repo
        .create_context(user_id, Some(session_id), "seed-context", ContextKind::User)
        .await
        .expect("create context");

    let task_id = TaskId::generate();
    let trace_id = TraceId::generate();
    let task = make_task(&task_id, &context_id);
    repos
        .tasks
        .create_task(systemprompt_agent::repository::task::RepoCreateTaskParams {
            task: &task,
            user_id,
            session_id,
            trace_id: &trace_id,
            agent_name: "test-agent",
        })
        .await
        .expect("create task");

    (context_id, task_id)
}

pub(crate) fn make_task(
    task_id: &TaskId,
    context_id: &ContextId,
) -> systemprompt_agent::models::a2a::Task {
    use systemprompt_agent::models::a2a::{Task, TaskState, TaskStatus};
    Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: None,
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}
