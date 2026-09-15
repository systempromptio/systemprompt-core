//! Core inventory orchestration uses configured services roots and
//! organizational ownership.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OptimizationError;
use crate::AppContext;
use std::sync::OnceLock;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::inventory::{
    BaselineCapture, BaselinePreparation, BaselineScope, InventoryService, InventoryStatus,
    PublishGuard,
};
pub use systemprompt_marketplace::inventory::{LatestPublication, LatestPublicationStatus};

// Why: the guard memoises per-entry tree digests across passes; one per
// process keeps the scheduled job and the manual route from re-capturing
// the same unchanged tree.
static PUBLISH_GUARD: OnceLock<tokio::sync::Mutex<PublishGuard>> = OnceLock::new();

pub async fn refresh(
    ctx: &AppContext,
    owner: &UserId,
) -> Result<InventoryStatus, OptimizationError> {
    let services = load_services(ctx, owner).await?;
    Ok(
        InventoryService::new(ctx.managed_repository().as_ref().clone())
            .refresh(owner, ctx.app_paths().system().services(), &services)
            .await?,
    )
}

pub async fn prepare_baselines(
    ctx: &AppContext,
    owner: &UserId,
    actor: &UserId,
    request: &BaselinePreparation,
) -> Result<Vec<BaselineCapture>, OptimizationError> {
    let services = load_services(ctx, owner).await?;
    let service = InventoryService::new(ctx.managed_repository().as_ref().clone());
    service
        .refresh(owner, ctx.app_paths().system().services(), &services)
        .await?;
    Ok(service
        .prepare_baselines(
            &BaselineScope {
                owner,
                actor,
                root: ctx.app_paths().system().services(),
                services: &services,
            },
            request,
        )
        .await?)
}

pub async fn publish_latest(
    ctx: &AppContext,
    owner: &UserId,
    actor: &UserId,
) -> Result<Vec<LatestPublication>, OptimizationError> {
    let services = load_services(ctx, owner).await?;
    let service = InventoryService::new(ctx.managed_repository().as_ref().clone());
    service
        .refresh(owner, ctx.app_paths().system().services(), &services)
        .await?;
    let mut guard = PUBLISH_GUARD
        .get_or_init(|| tokio::sync::Mutex::new(PublishGuard::default()))
        .lock()
        .await;
    Ok(service
        .publish_latest(
            &BaselineScope {
                owner,
                actor,
                root: ctx.app_paths().system().services(),
                services: &services,
            },
            &mut guard,
        )
        .await?)
}

async fn load_services(
    ctx: &AppContext,
    owner: &UserId,
) -> Result<systemprompt_models::services::ServicesConfig, OptimizationError> {
    match systemprompt_loader::ConfigLoader::load() {
        Ok(services) => Ok(services),
        Err(_error) => {
            ctx.managed_repository()
                .record_inventory_failure(owner)
                .await?;
            Err(OptimizationError::Source(
                "Configured inventory could not be loaded; previous inventory retained".to_owned(),
            ))
        },
    }
}
