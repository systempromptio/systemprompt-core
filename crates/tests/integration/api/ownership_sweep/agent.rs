use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use axum::{Extension, Router};
use systemprompt_api::routes::proxy::mcp;
use systemprompt_api::routes::{artifacts_router, contexts_router, tasks_router, webhook_router};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use uuid::Uuid;

use super::engine::{Fixture, get, sweep};
use super::table::Family;
use crate::common::setup_ctx;

struct Seeded {
    ctx: Arc<AppContext>,
    owner: UserId,
    session: SessionId,
    stranger: UserId,
    context: ContextId,
    task: TaskId,
    artifact: ArtifactId,
    execution: String,
}

fn caller(user: &UserId, session: SessionId) -> RequestContext {
    RequestContext::new(
        session,
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("sweep-agent").expect("agent name"),
        Actor::user(user.clone()),
    )
    .with_user_type(UserType::User)
}

async fn seed_user(pool: &DbPool, label: &str) -> Result<UserId> {
    let user = unique_user_id(label);
    seed_user_row(pool, &user, &format!("{}@sweep.invalid", user.as_str())).await?;
    Ok(user)
}

async fn seed() -> Result<Seeded> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_user(&pool, "sweep-agent-owner").await?;
    let stranger = seed_user(&pool, "sweep-agent-stranger").await?;
    let session = SessionId::generate();
    seed_user_session(&pool, &owner, &session).await?;
    let context = ContextId::generate();
    let task = TaskId::generate();
    let artifact = ArtifactId::generate();
    let execution = Uuid::new_v4().to_string();
    let handle = pool.pool();
    let db = handle.as_ref();
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, \
         'sweep-context')",
    )
    .bind(context.as_str())
    .bind(owner.as_str())
    .bind(session.as_str())
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, \
         agent_name) VALUES ($1, $2, 'TASK_STATE_WORKING', now(), $3, 'sweep-agent')",
    )
    .bind(task.as_str())
    .bind(context.as_str())
    .bind(owner.as_str())
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO task_artifacts (task_id, context_id, artifact_id, name, artifact_type) \
         VALUES ($1, $2, $3, 'sweep artifact', 'table')",
    )
    .bind(task.as_str())
    .bind(context.as_str())
    .bind(artifact.as_str())
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, \
         data_content) VALUES ($1, $2, 'data', 0, '{\"columns\":[\"n\"],\"rows\":[[\"a\"]]}'::jsonb)",
    )
    .bind(artifact.as_str())
    .bind(context.as_str())
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         input, output, status, user_id) VALUES ($1, 'lookup', 'files', CURRENT_TIMESTAMP, '{}', \
         '{}', 'success', $2)",
    )
    .bind(&execution)
    .bind(owner.as_str())
    .execute(db)
    .await?;
    Ok(Seeded {
        ctx,
        owner,
        session,
        stranger,
        context,
        task,
        artifact,
        execution,
    })
}

struct Scoped<'a> {
    seeded: &'a Seeded,
    resource: String,
    view: String,
    mount: fn(&AppContext) -> Router,
    view_mount: fn(&AppContext) -> Router,
}

impl Fixture for Scoped<'_> {
    fn owner(&self) -> Router {
        (self.mount)(&self.seeded.ctx).layer(Extension(caller(
            &self.seeded.owner,
            self.seeded.session.clone(),
        )))
    }

    fn stranger(&self) -> Router {
        (self.mount)(&self.seeded.ctx).layer(Extension(caller(
            &self.seeded.stranger,
            SessionId::generate(),
        )))
    }

    fn resource(&self) -> &str {
        &self.resource
    }

    fn owner_view(&self) -> Request<Body> {
        get(&self.view)
    }

    fn view_router(&self) -> Router {
        (self.view_mount)(&self.seeded.ctx).layer(Extension(caller(
            &self.seeded.owner,
            self.seeded.session.clone(),
        )))
    }

    fn stranger_id(&self) -> String {
        self.seeded.stranger.as_str().to_owned()
    }
}

fn contexts(ctx: &AppContext) -> Router {
    contexts_router().with_state(ctx.clone())
}

fn tasks(ctx: &AppContext) -> Router {
    tasks_router().with_state(ctx.clone())
}

fn artifacts(ctx: &AppContext) -> Router {
    artifacts_router().with_state(ctx.clone())
}

fn webhooks(ctx: &AppContext) -> Router {
    webhook_router().with_state(ctx.clone())
}

fn executions(ctx: &AppContext) -> Router {
    mcp::executions_router(ctx)
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_context_route() -> Result<()> {
    let seeded = seed().await?;
    let id = seeded.context.as_str().to_owned();
    let fixture = Scoped {
        seeded: &seeded,
        view: format!("/{id}"),
        resource: id,
        mount: contexts,
        view_mount: contexts,
    };
    sweep(&fixture, Family::Contexts).await
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_task_route() -> Result<()> {
    let seeded = seed().await?;
    let id = seeded.task.as_str().to_owned();
    let fixture = Scoped {
        seeded: &seeded,
        view: format!("/{id}"),
        resource: id,
        mount: tasks,
        view_mount: tasks,
    };
    sweep(&fixture, Family::Tasks).await
}

#[tokio::test]
async fn a_second_user_is_refused_every_owned_artifact_route() -> Result<()> {
    let seeded = seed().await?;
    let id = seeded.artifact.as_str().to_owned();
    let fixture = Scoped {
        seeded: &seeded,
        view: format!("/{id}"),
        resource: id,
        mount: artifacts,
        view_mount: artifacts,
    };
    sweep(&fixture, Family::Artifacts).await
}

#[tokio::test]
async fn a_second_user_is_refused_an_owned_mcp_execution() -> Result<()> {
    let seeded = seed().await?;
    let id = seeded.execution.clone();
    let fixture = Scoped {
        seeded: &seeded,
        view: format!("/executions/{id}"),
        resource: id,
        mount: executions,
        view_mount: executions,
    };
    sweep(&fixture, Family::McpExecutions).await
}

#[tokio::test]
async fn a_second_user_cannot_broadcast_into_another_users_context() -> Result<()> {
    let seeded = seed().await?;
    let id = seeded.context.as_str().to_owned();
    let fixture = Scoped {
        seeded: &seeded,
        view: format!("/{id}"),
        resource: id,
        mount: webhooks,
        view_mount: contexts,
    };
    sweep(&fixture, Family::Webhooks).await
}
