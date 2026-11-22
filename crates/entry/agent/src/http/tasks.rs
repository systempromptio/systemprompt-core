//! Agent task listing and lookup endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_identifiers::TaskId;

use systemprompt_agent::models::a2a::TaskState;
use systemprompt_agent::repository::task::ListTasksFilter;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;

use super::clamp_page_limit;
use crate::kernel::ApiHttpError;

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

    let context_id_typed = super::parse_context_id(&context_id)?;

    let context_repo = app_context.a2a_repositories().contexts.clone();
    let owned = context_repo
        .owned_context(&context_id_typed, req_ctx.user_id())
        .await?;

    let task_repo = app_context.a2a_repositories().tasks.clone();
    let tasks = task_repo.list_tasks_by_context(&owned).await?;

    tracing::debug!(context_id = %context_id, count = %tasks.len(), "Tasks listed");
    Ok((StatusCode::OK, Json(tasks)))
}

pub async fn get_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Retrieving task");

    let task_repo = app_context.a2a_repositories().tasks.clone();

    let task_id_typed = TaskId::try_new(&task_id).map_err(ApiError::from)?;
    let owned = task_repo
        .owned_task(&task_id_typed, req_ctx.user_id())
        .await?;
    let task = task_repo.find_owned_task(&owned).await?;

    tracing::debug!("Task retrieved successfully");
    Ok((StatusCode::OK, Json(task)))
}

pub async fn list_tasks_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<TaskFilterParams>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let user_id = req_ctx.user_id();

    tracing::debug!(user_id = %user_id, "Listing tasks");

    let task_repo = app_context.a2a_repositories().tasks.clone();

    let task_state = match params.status.as_deref() {
        // Why: the parse error repeats the rejected value and nothing else, so
        // the 400 names it directly.
        Some(status) => Some(status.parse::<TaskState>().map_err(|_unknown| {
            ApiHttpError::bad_request(format!("unknown task status {status:?}"))
        })?),
        None => None,
    };

    let filter = ListTasksFilter {
        user_id,
        context_id: None,
        status: task_state,
        status_timestamp_after: None,
    };
    let tasks = task_repo
        .list_tasks(filter, i64::from(clamp_page_limit(params.limit)), 0)
        .await?;

    tracing::debug!(user_id = %user_id, count = %tasks.len(), "Tasks listed");
    Ok((StatusCode::OK, Json(tasks)))
}

pub async fn get_messages_by_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Retrieving messages");

    let task_repo = app_context.a2a_repositories().tasks.clone();

    let task_id_typed = TaskId::try_new(&task_id).map_err(ApiError::from)?;
    let owned = task_repo
        .owned_task(&task_id_typed, req_ctx.user_id())
        .await?;
    let messages = task_repo.owned_task_messages(&owned).await?;

    tracing::debug!(task_id = %task_id, count = %messages.len(), "Messages retrieved");
    Ok((StatusCode::OK, Json(messages)))
}

pub async fn delete_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Deleting task");

    let task_repo = app_context.a2a_repositories().tasks.clone();

    let task_id_typed = TaskId::try_new(&task_id).map_err(ApiError::from)?;
    let owned = task_repo
        .owned_task(&task_id_typed, req_ctx.user_id())
        .await?;
    task_repo.delete_task(&owned).await?;

    tracing::debug!(task_id = %task_id, "Task deleted");
    Ok(StatusCode::NO_CONTENT)
}
