use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::Extension;
use serde::Deserialize;
use systemprompt_identifiers::{ContextId, TaskId, UserId};

use crate::models::a2a::TaskState;
use crate::repository::task::TaskRepository;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

#[derive(Debug, Deserialize)]
pub struct TaskFilterParams {
    status: Option<String>,
    limit: Option<u32>,
}

pub async fn list_tasks_by_context(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(context_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(context_id = %context_id, "Listing tasks");

    let task_repo = TaskRepository::new(app_context.db_pool().clone());

    let context_id_typed = ContextId::new(&context_id);
    match task_repo.list_tasks_by_context(&context_id_typed).await {
        Ok(tasks) => {
            tracing::debug!(context_id = %context_id, count = %tasks.len(), "Tasks listed");
            (StatusCode::OK, Json(tasks)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list tasks");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve tasks",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn get_task(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(task_id = %task_id, "Retrieving task");

    let task_repo = TaskRepository::new(app_context.db_pool().clone());

    let task_id_typed = TaskId::new(&task_id);
    match task_repo.get_task(&task_id_typed).await {
        Ok(Some(task)) => {
            tracing::debug!("Task retrieved successfully");
            (StatusCode::OK, Json(task)).into_response()
        },
        Ok(None) => {
            tracing::debug!("Task not found");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Task not found",
                    "task_id": task_id
                })),
            )
                .into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to retrieve task");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve task",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn list_tasks_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<TaskFilterParams>,
) -> impl IntoResponse {
    let user_id = req_ctx.auth.user_id.as_str();

    tracing::debug!(user_id = %user_id, "Listing tasks");

    let task_repo = TaskRepository::new(app_context.db_pool().clone());

    let task_state = params.status.as_ref().and_then(|s| match s.as_str() {
        "submitted" => Some(TaskState::Submitted),
        "working" => Some(TaskState::Working),
        "input-required" => Some(TaskState::InputRequired),
        "completed" => Some(TaskState::Completed),
        "canceled" => Some(TaskState::Canceled),
        "cancelled" => Some(TaskState::Canceled),
        "failed" => Some(TaskState::Failed),
        "rejected" => Some(TaskState::Rejected),
        "auth-required" => Some(TaskState::AuthRequired),
        _ => None,
    });

    let user_id_typed = UserId::new(user_id);
    match task_repo
        .get_tasks_by_user_id(&user_id_typed, params.limit.map(|l| l as i32), None)
        .await
    {
        Ok(mut tasks) => {
            if let Some(state) = task_state {
                tasks.retain(|t| t.status.state == state);
            }

            tracing::debug!(user_id = %user_id, count = %tasks.len(), "Tasks listed");
            (StatusCode::OK, Json(tasks)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list tasks");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve tasks",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn get_messages_by_task(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(task_id = %task_id, "Retrieving messages");

    let task_repo = TaskRepository::new(app_context.db_pool().clone());

    let task_id_typed = TaskId::new(&task_id);
    match task_repo.get_messages_by_task(&task_id_typed).await {
        Ok(messages) => {
            tracing::debug!(task_id = %task_id, count = %messages.len(), "Messages retrieved");
            (StatusCode::OK, Json(messages)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to retrieve messages");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve messages",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn delete_task(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(task_id = %task_id, "Deleting task");

    let task_repo = TaskRepository::new(app_context.db_pool().clone());

    let task_id_typed = TaskId::new(&task_id);
    match task_repo.delete_task(&task_id_typed).await {
        Ok(()) => {
            tracing::debug!(task_id = %task_id, "Task deleted");
            StatusCode::NO_CONTENT.into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete task");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to delete task",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}
