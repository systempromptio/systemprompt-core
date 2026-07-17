//! Agent task listing and lookup endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_identifiers::{ContextId, TaskId, UserId};

use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;

#[derive(Debug, Deserialize)]
pub struct TaskFilterParams {
    pub status: Option<String>,
    pub limit: Option<u32>,
}

pub async fn list_tasks_by_context(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(context_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(context_id = %context_id, "Listing tasks");

    let context_id_typed = ContextId::new(&context_id);

    let context_repo = ContextRepository::new(app_context.db_pool())?;
    context_repo
        .validate_context_ownership(&context_id_typed, req_ctx.user_id())
        .await?;

    let task_repo = TaskRepository::new(app_context.db_pool())?;
    let tasks = task_repo.list_tasks_by_context(&context_id_typed).await?;

    tracing::debug!(context_id = %context_id, count = %tasks.len(), "Tasks listed");
    Ok((StatusCode::OK, Json(tasks)))
}

pub async fn get_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Retrieving task");

    let task_repo = TaskRepository::new(app_context.db_pool())?;

    let task_id_typed = TaskId::new(&task_id);
    task_repo
        .validate_task_ownership(&task_id_typed, req_ctx.user_id())
        .await?;

    let task = task_repo
        .get_task(&task_id_typed)
        .await?
        .ok_or_else(|| ApiHttpError::not_found(format!("Task '{task_id}' not found")))?;

    tracing::debug!("Task retrieved successfully");
    Ok((StatusCode::OK, Json(task)))
}

pub async fn list_tasks_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<TaskFilterParams>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let user_id = req_ctx.auth.actor.user_id.as_str();

    tracing::debug!(user_id = %user_id, "Listing tasks");

    let task_repo = TaskRepository::new(app_context.db_pool())?;

    let task_state = params.status.as_ref().and_then(|s| match s.as_str() {
        "submitted" => Some(TaskState::Submitted),
        "working" => Some(TaskState::Working),
        "input-required" => Some(TaskState::InputRequired),
        "completed" => Some(TaskState::Completed),
        "canceled" | "cancelled" => Some(TaskState::Canceled),
        "failed" => Some(TaskState::Failed),
        "rejected" => Some(TaskState::Rejected),
        "auth-required" => Some(TaskState::AuthRequired),
        _ => None,
    });

    let user_id_typed = UserId::new(user_id);
    let mut tasks = task_repo
        .get_tasks_by_user_id(&user_id_typed, params.limit.map(|l| l as i32), None)
        .await?;

    if let Some(state) = task_state {
        tasks.retain(|t| t.status.state == state);
    }

    tracing::debug!(user_id = %user_id, count = %tasks.len(), "Tasks listed");
    Ok((StatusCode::OK, Json(tasks)))
}

pub async fn get_messages_by_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Retrieving messages");

    let task_repo = TaskRepository::new(app_context.db_pool())?;

    let task_id_typed = TaskId::new(&task_id);
    task_repo
        .validate_task_ownership(&task_id_typed, req_ctx.user_id())
        .await?;

    let messages = task_repo.get_messages_by_task(&task_id_typed).await?;

    tracing::debug!(task_id = %task_id, count = %messages.len(), "Messages retrieved");
    Ok((StatusCode::OK, Json(messages)))
}

pub async fn delete_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Deleting task");

    let task_repo = TaskRepository::new(app_context.db_pool())?;

    let task_id_typed = TaskId::new(&task_id);
    task_repo
        .validate_task_ownership(&task_id_typed, req_ctx.user_id())
        .await?;

    task_repo.delete_task(&task_id_typed).await?;

    tracing::debug!(task_id = %task_id, "Task deleted");
    Ok(StatusCode::NO_CONTENT)
}
