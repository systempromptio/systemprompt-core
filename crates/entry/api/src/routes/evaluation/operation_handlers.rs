//! Retained input checkpoints make source capture and refresh restart-safe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::operations::{self, OperationResponse};
use super::optimization_error::OptimizationHttpError;
use super::optimization_state::OptimizationState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use systemprompt_identifiers::ManagedSourceId;
use systemprompt_marketplace::inventory::{ConfiguredInventoryEntry, InventoryStatus};
use systemprompt_marketplace::managed::operations::ApiOperationClaim;
use systemprompt_marketplace::managed::{CapturedSkills, ImportedSkills};
use systemprompt_models::feedback::verification::{
    DependencyVerificationManifest, DependencyVerificationRequest,
};
use systemprompt_runtime::AppContext;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CaptureSource {
    pub skill_ids: Vec<String>,
}

pub(super) async fn refresh(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let claim = operations::begin(&ctx, &headers, "inventory_refresh", &()).await?;
    let response: OperationResponse<InventoryStatus> = match claim {
        ApiOperationClaim::Retained(operation) => operations::response(&operation)?,
        ApiOperationClaim::Acquired(operation) => {
            let result: Result<InventoryStatus, OptimizationHttpError> = async {
                let configured = if let Some(configured) = ctx
                    .managed_repository()
                    .api_input::<Vec<ConfiguredInventoryEntry>>(ctx.system_admin().id(), &operation)
                    .await?
                {
                    configured
                } else {
                    let services = systemprompt_loader::ConfigLoader::load().map_err(|error| {
                        systemprompt_evaluation::EvaluationError::InvalidSpec(format!(
                            "Configured inventory unavailable: {error}"
                        ))
                    })?;
                    let root = ctx.app_paths().system().services().to_path_buf();
                    let configured = tokio::task::spawn_blocking(move || {
                        systemprompt_marketplace::inventory::scan_configured_inventory(
                            &root, &services,
                        )
                    })
                    .await
                    .map_err(|error| {
                        systemprompt_evaluation::EvaluationError::InvalidSpec(format!(
                            "Inventory scan unavailable: {error}"
                        ))
                    })??;
                    ctx.managed_repository()
                        .checkpoint_api_input(ctx.system_admin().id(), &operation, &configured)
                        .await?
                };
                Ok(ctx
                    .managed_repository()
                    .reconcile_inventory_operation(
                        ctx.system_admin().id(),
                        &configured,
                        Some(&operation),
                    )
                    .await?)
            }
            .await;
            match result {
                Ok(_) => operations::response(
                    &ctx.managed_repository()
                        .api_operation(ctx.system_admin().id(), &operation.id)
                        .await?,
                )?,
                Err(error) => {
                    ctx.managed_repository()
                        .fail_api_operation(ctx.system_admin().id(), &operation)
                        .await?;
                    return Err(error);
                },
            }
        },
    };
    Ok((
        operations::status_code(&response),
        [(
            "location",
            format!("/api/v1/operations/{}", response.operation.id),
        )],
        Json(response),
    ))
}
pub(super) async fn capture(
    State(state): State<OptimizationState>,
    headers: HeaderMap,
    Path(source): Path<ManagedSourceId>,
    Json(input): Json<CaptureSource>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let ctx = state.ctx();
    if input.skill_ids.is_empty()
        || input.skill_ids.len() > 100
        || input
            .skill_ids
            .iter()
            .any(|id| id.is_empty() || id.len() > 180)
    {
        return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
            "Capture requires 1–100 bounded skill identifiers".to_owned(),
        )
        .into());
    }
    let claim = operations::begin(ctx, &headers, "source_capture", &(&source, &input)).await?;
    let response: OperationResponse<ImportedSkills> = match claim {
        ApiOperationClaim::Retained(operation) => operations::response(&operation)?,
        ApiOperationClaim::Acquired(operation) => {
            let result = async {
                let captured = if let Some(captured) = ctx
                    .managed_repository()
                    .api_input::<CapturedSkills>(ctx.system_admin().id(), &operation)
                    .await?
                {
                    captured
                } else {
                    let captured = state
                        .orchestrator()
                        .capture_authoring_input(
                            ctx.system_admin().id(),
                            &source,
                            ctx.app_paths().system().services(),
                            input.skill_ids,
                        )
                        .await?;
                    ctx.managed_repository()
                        .checkpoint_api_input(ctx.system_admin().id(), &operation, &captured)
                        .await?
                };
                Ok(ctx
                    .managed_repository()
                    .import_api_capture(ctx.system_admin().id(), &operation, &source, &captured)
                    .await?)
            }
            .await;
            operations::finish(ctx, &operation, result).await?
        },
    };
    Ok((
        operations::status_code(&response),
        [(
            "location",
            format!("/api/v1/operations/{}", response.operation.id),
        )],
        Json(response),
    ))
}
pub(super) async fn verify(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(input): Json<DependencyVerificationRequest>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    input.validate().map_err(|error| {
        systemprompt_evaluation::EvaluationError::InvalidSpec(format!(
            "Dependency verification manifest is incomplete: {error}"
        ))
    })?;
    let claim = operations::begin(&ctx, &headers, "source_verification", &input).await?;
    let response: OperationResponse<DependencyVerificationManifest> = match claim {
        ApiOperationClaim::Retained(operation) => operations::response(&operation)?,
        ApiOperationClaim::Acquired(operation) => {
            let result =
                systemprompt_runtime::optimization::git_sources::GitSourceOrchestrator::new(
                    ctx.managed_repository().as_ref().clone(),
                )
                .verify(ctx.system_admin().id(), &input)
                .await
                .map_err(Into::into);
            operations::finish(&ctx, &operation, result).await?
        },
    };
    Ok((
        operations::status_code(&response),
        [(
            "location",
            format!("/api/v1/operations/{}", response.operation.id),
        )],
        Json(response),
    ))
}
