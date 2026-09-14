//! Administrative inventory, observed membership and retained baseline
//! preparation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::optimization_error::OptimizationHttpError;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{InventoryEntryId, ManagedResourceId, TaskId};
use systemprompt_marketplace::inventory::{
    BaselineCapture, BaselinePreparation, InventoryEntry, InventoryStatus, ObservedMembership,
};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::optimization::inventory;

pub(super) fn router() -> Router<AppContext> {
    Router::new()
        .route("/inventory", get(list))
        .route("/inventory/status", get(status))
        .route("/inventory/installations/status", get(installation_status))
        .route(
            "/inventory/{id}/installation-coverage",
            get(installation_coverage),
        )
        .route("/inventory/reconciliations", post(refresh))
        .route("/inventory/baselines", post(baselines))
        .route("/inventory/{id}", get(entry))
        .route("/inventory/{id}/bindings", post(bind))
        .route("/inventory/{id}/membership", get(membership))
        .route("/inventory/{id}/reconciliations", get(reconciliations))
        .route("/inventory/{id}/git-binding", get(git_binding))
        .route("/inventory/{id}/captures/{operation}", get(capture))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    after: Option<InventoryEntryId>,
    limit: Option<u32>,
}
#[derive(Debug, Serialize)]
struct Page {
    items: Vec<InventoryEntry>,
    next_cursor: Option<InventoryEntryId>,
}

async fn list(
    State(ctx): State<AppContext>,
    Query(query): Query<Cursor>,
) -> Result<Json<Page>, OptimizationHttpError> {
    let limit = query.limit.unwrap_or(50);
    let items = ctx
        .managed_repository()
        .inventory(ctx.system_admin().id(), query.after.as_ref(), limit)
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items.last().map(|entry| entry.entry_id.clone())
    } else {
        None
    };
    Ok(Json(Page { items, next_cursor }))
}
async fn status(
    State(ctx): State<AppContext>,
) -> Result<Json<InventoryStatus>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_status(ctx.system_admin().id())
            .await?,
    ))
}
async fn entry(
    State(ctx): State<AppContext>,
    Path(id): Path<InventoryEntryId>,
) -> Result<Json<InventoryEntry>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_entry(ctx.system_admin().id(), &id)
            .await?,
    ))
}
async fn refresh(
    State(ctx): State<AppContext>,
) -> Result<Json<InventoryStatus>, OptimizationHttpError> {
    Ok(Json(
        inventory::refresh(&ctx, ctx.system_admin().id()).await?,
    ))
}
async fn baselines(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Json(input): Json<BaselinePreparation>,
) -> Result<Json<Vec<BaselineCapture>>, OptimizationHttpError> {
    Ok(Json(
        inventory::prepare_baselines(&ctx, ctx.system_admin().id(), actor.user_id(), &input)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    resource_id: ManagedResourceId,
}
async fn bind(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Path(id): Path<InventoryEntryId>,
    Json(input): Json<Binding>,
) -> Result<StatusCode, OptimizationHttpError> {
    ctx.managed_repository()
        .bind_inventory_resource(
            ctx.system_admin().id(),
            actor.user_id(),
            &id,
            &input.resource_id,
        )
        .await?;
    inventory::refresh(&ctx, ctx.system_admin().id()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MembershipTime {
    at: chrono::DateTime<chrono::Utc>,
}
async fn membership(
    State(ctx): State<AppContext>,
    Path(id): Path<InventoryEntryId>,
    Query(query): Query<MembershipTime>,
) -> Result<Json<ObservedMembership>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_membership(ctx.system_admin().id(), &id, query.at)
            .await?,
    ))
}
async fn capture(
    State(ctx): State<AppContext>,
    Path((id, operation)): Path<(InventoryEntryId, TaskId)>,
) -> Result<Json<BaselineCapture>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_capture(ctx.system_admin().id(), &id, &operation)
            .await?
            .ok_or(systemprompt_marketplace::managed::ManagedError::Unavailable)?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationCursor {
    after: Option<systemprompt_identifiers::ManagedReconciliationId>,
}
async fn reconciliations(
    State(ctx): State<AppContext>,
    Path(id): Path<InventoryEntryId>,
    Query(query): Query<ReconciliationCursor>,
) -> Result<
    Json<Vec<systemprompt_marketplace::inventory::InventoryReconciliation>>,
    OptimizationHttpError,
> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_reconciliations(ctx.system_admin().id(), &id, query.after.as_ref())
            .await?,
    ))
}
async fn git_binding(
    State(ctx): State<AppContext>,
    Path(id): Path<InventoryEntryId>,
) -> Result<
    Json<Option<systemprompt_marketplace::inventory::InventoryGitBinding>>,
    OptimizationHttpError,
> {
    Ok(Json(
        ctx.managed_repository()
            .inventory_git_binding(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn installation_status(
    State(ctx): State<AppContext>,
) -> Result<
    Json<systemprompt_marketplace::inventory::InstallationCoverageStatus>,
    OptimizationHttpError,
> {
    Ok(Json(
        ctx.managed_repository()
            .installation_coverage_status(ctx.system_admin().id())
            .await?,
    ))
}
async fn installation_coverage(
    State(ctx): State<AppContext>,
    Path(id): Path<InventoryEntryId>,
) -> Result<
    Json<Option<systemprompt_marketplace::inventory::InstallationCoverage>>,
    OptimizationHttpError,
> {
    let entry = ctx
        .managed_repository()
        .inventory_entry(ctx.system_admin().id(), &id)
        .await?;
    let coverage = if let Some(resource) = entry.resource_id {
        ctx.managed_repository()
            .installation_coverage(ctx.system_admin().id(), &resource)
            .await?
    } else {
        None
    };
    Ok(Json(coverage))
}
