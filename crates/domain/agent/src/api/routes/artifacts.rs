use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_models::api::ApiError;

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
) -> Result<impl IntoResponse, ApiError> {
    tracing::debug!(context_id = %context_id, "Listing artifacts by context");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let context_id_typed = ContextId::new(&context_id);
    let artifacts = artifact_repo
        .get_artifacts_by_context(&context_id_typed)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to list artifacts");
            ApiError::internal_error("Failed to retrieve artifacts")
        })?;

    tracing::debug!(
        context_id = %context_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}

pub async fn list_artifacts_by_task(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::debug!(task_id = %task_id, "Listing artifacts by task");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let task_id_typed = TaskId::new(&task_id);
    let artifacts = artifact_repo
        .get_artifacts_by_task(&task_id_typed)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to list artifacts");
            ApiError::internal_error("Failed to retrieve artifacts")
        })?;

    tracing::debug!(
        task_id = %task_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}

pub async fn get_artifact(
    Extension(_req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(artifact_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::debug!(artifact_id = %artifact_id, "Retrieving artifact");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let artifact_id_typed = ArtifactId::new(&artifact_id);
    match artifact_repo.get_artifact_by_id(&artifact_id_typed).await {
        Ok(Some(artifact)) => {
            tracing::debug!("Artifact retrieved successfully");
            Ok((StatusCode::OK, Json(artifact)).into_response())
        },
        Ok(None) => {
            tracing::debug!("Artifact not found");
            Err(ApiError::not_found(format!(
                "Artifact '{}' not found",
                artifact_id
            )))
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to retrieve artifact");
            Err(ApiError::internal_error("Failed to retrieve artifact"))
        },
    }
}

pub async fn list_artifacts_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<ArtifactQueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = req_ctx.auth.user_id.as_str();

    tracing::debug!(user_id = %user_id, "Listing artifacts by user");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool().clone());

    let user_id_typed = UserId::new(user_id);
    let artifacts = artifact_repo
        .get_artifacts_by_user_id(&user_id_typed, params.limit.map(|l| l as i32))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to list artifacts");
            ApiError::internal_error("Failed to retrieve artifacts")
        })?;

    tracing::debug!(
        user_id = %user_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}
