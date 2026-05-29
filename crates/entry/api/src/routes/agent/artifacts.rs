use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_mcp::services::ui_renderer::MCP_APP_MIME_TYPE;
use systemprompt_mcp::services::ui_renderer::registry::create_default_registry;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ArtifactQueryParams {
    pub limit: Option<u32>,
}

pub async fn list_artifacts_by_context(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(context_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(context_id = %context_id, "Listing artifacts by context");

    let context_id_typed = ContextId::new(&context_id);

    let context_repo = ContextRepository::new(app_context.db_pool())?;
    context_repo
        .validate_context_ownership(&context_id_typed, req_ctx.user_id())
        .await?;

    let artifact_repo = ArtifactRepository::new(app_context.db_pool())?;
    let artifacts = artifact_repo
        .get_artifacts_by_context(&context_id_typed)
        .await?;

    tracing::debug!(
        context_id = %context_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}

pub async fn list_artifacts_by_task(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(task_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(task_id = %task_id, "Listing artifacts by task");

    let task_id_typed = TaskId::new(&task_id);

    let task_repo = TaskRepository::new(app_context.db_pool())?;
    task_repo
        .validate_task_ownership(&task_id_typed, req_ctx.user_id())
        .await?;

    let artifact_repo = ArtifactRepository::new(app_context.db_pool())?;
    let artifacts = artifact_repo.get_artifacts_by_task(&task_id_typed).await?;

    tracing::debug!(
        task_id = %task_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}

pub async fn get_artifact(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(artifact_id): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    tracing::debug!(artifact_id = %artifact_id, "Retrieving artifact");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool())?;

    let artifact_id_typed = ArtifactId::new(&artifact_id);
    artifact_repo
        .validate_artifact_ownership(&artifact_id_typed, req_ctx.user_id())
        .await?;

    let artifact = artifact_repo
        .get_artifact_by_id(&artifact_id_typed)
        .await?
        .ok_or_else(|| ApiHttpError::not_found(format!("Artifact '{artifact_id}' not found")))?;

    tracing::debug!("Artifact retrieved successfully");
    Ok((StatusCode::OK, Json(artifact)))
}

pub async fn list_artifacts_by_user(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Query(params): Query<ArtifactQueryParams>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let user_id = req_ctx.auth.actor.user_id.as_str();

    tracing::debug!(user_id = %user_id, "Listing artifacts by user");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool())?;

    let user_id_typed = UserId::new(user_id);
    let artifacts = artifact_repo
        .get_artifacts_by_user_id(&user_id_typed, params.limit.map(|l| l as i32))
        .await?;

    tracing::debug!(
        user_id = %user_id,
        count = artifacts.len(),
        "Artifacts listed"
    );
    Ok((StatusCode::OK, Json(artifacts)))
}

pub async fn get_artifact_ui(
    Extension(req_ctx): Extension<RequestContext>,
    State(app_context): State<AppContext>,
    Path(artifact_id): Path<String>,
) -> Result<Response, ApiHttpError> {
    tracing::debug!(artifact_id = %artifact_id, "Rendering artifact as MCP App UI");

    let artifact_repo = ArtifactRepository::new(app_context.db_pool())?;
    let artifact_id_typed = ArtifactId::new(&artifact_id);

    artifact_repo
        .validate_artifact_ownership(&artifact_id_typed, req_ctx.user_id())
        .await?;

    let artifact = artifact_repo
        .get_artifact_by_id(&artifact_id_typed)
        .await?
        .ok_or_else(|| ApiHttpError::not_found(format!("Artifact '{artifact_id}' not found")))?;

    let registry = create_default_registry();
    let artifact_type = &artifact.metadata.artifact_type;

    if !registry.supports(artifact_type) {
        tracing::warn!(artifact_type = %artifact_type, "No UI renderer for artifact type");
        return Err(ApiHttpError::bad_request(format!(
            "No UI renderer available for artifact type '{artifact_type}'"
        )));
    }

    let ui_resource: systemprompt_mcp::services::ui_renderer::UiResource = registry
        .render(&artifact)
        .await
        .map_err(|e| ApiHttpError::internal_error(format!("Failed to render artifact UI: {e}")))?;

    tracing::debug!(artifact_id = %artifact_id, "Artifact UI rendered successfully");

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, MCP_APP_MIME_TYPE)
        .header(
            header::CONTENT_SECURITY_POLICY,
            ui_resource.csp.to_header_value(),
        )
        .header(header::X_FRAME_OPTIONS, "SAMEORIGIN")
        .body(axum::body::Body::from(ui_resource.html))
        .map_err(|e| ApiHttpError::internal_error(format!("Failed to build response: {e}")))
}
