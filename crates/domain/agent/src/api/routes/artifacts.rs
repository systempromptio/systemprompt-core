use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::Extension;
use serde::Deserialize;

use crate::repository::content::ArtifactRepository;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ArtifactQueryParams {
    limit: Option<u32>,
}

pub async fn list_artifacts_by_context(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(context_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(context_id = %context_id, "Listing artifacts by context");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let context_id_typed = ContextId::new(&context_id);
    match artifact_repo
        .get_artifacts_by_context(&context_id_typed)
        .await
    {
        Ok(artifacts) => {
            tracing::debug!(
                context_id = %context_id,
                count = artifacts.len(),
                "Artifacts listed"
            );
            (StatusCode::OK, Json(artifacts)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list artifacts");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve artifacts",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn list_artifacts_by_task(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(task_id = %task_id, "Listing artifacts by task");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let task_id_typed = TaskId::new(&task_id);
    match artifact_repo.get_artifacts_by_task(&task_id_typed).await {
        Ok(artifacts) => {
            tracing::debug!(
                task_id = %task_id,
                count = artifacts.len(),
                "Artifacts listed"
            );
            (StatusCode::OK, Json(artifacts)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list artifacts");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve artifacts",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn get_artifact(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(artifact_id): Path<String>,
) -> impl IntoResponse {
    tracing::debug!(artifact_id = %artifact_id, "Retrieving artifact");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let artifact_id_typed = ArtifactId::new(&artifact_id);
    match artifact_repo.get_artifact_by_id(&artifact_id_typed).await {
        Ok(Some(artifact)) => {
            tracing::debug!("Artifact retrieved successfully");
            (StatusCode::OK, Json(artifact)).into_response()
        },
        Ok(None) => {
            tracing::debug!("Artifact not found");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Artifact not found",
                    "artifact_id": artifact_id
                })),
            )
                .into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to retrieve artifact");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve artifact",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}

pub async fn list_artifacts_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<ArtifactQueryParams>,
) -> impl IntoResponse {
    let user_id = req_ctx.auth.user_id.as_str();

    tracing::debug!(user_id = %user_id, "Listing artifacts by user");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let user_id_typed = UserId::new(user_id);
    match artifact_repo
        .get_artifacts_by_user_id(&user_id_typed, params.limit.map(|l| l as i32))
        .await
    {
        Ok(artifacts) => {
            tracing::debug!(
                user_id = %user_id,
                count = artifacts.len(),
                "Artifacts listed"
            );
            (StatusCode::OK, Json(artifacts)).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list artifacts");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve artifacts",
                    "message": e.to_string()
                })),
            )
                .into_response()
        },
    }
}
