//! Core inventory orchestration over configured services roots.
//!
//! Ownership is organizational. The managed repository and the publish guard
//! are the ones the `managed_resources` member provides into the kernel state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::OrchestrationError;
use crate::AppContext;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::inventory::{
    BaselineScope, InventoryService, InventoryStatus, PublishGuard, SourcesContext,
};
pub use systemprompt_marketplace::inventory::{
    LatestPublication, LatestPublicationStatus, configured_inventory_fingerprint,
};
use systemprompt_marketplace::managed::ManagedRepository;
use tokio::sync::Mutex;

fn service(ctx: &AppContext) -> Result<InventoryService, OrchestrationError> {
    let repository = ctx.kernel().state().get::<Arc<ManagedRepository>>()?;
    Ok(InventoryService::new(repository.as_ref().clone()))
}

fn sources(ctx: &AppContext) -> SourcesContext<'_> {
    SourcesContext {
        profile: ctx.profile(),
        active_root: ctx.boot().services_root(),
    }
}

pub async fn refresh(
    ctx: &AppContext,
    owner: &UserId,
) -> Result<InventoryStatus, OrchestrationError> {
    let services = ctx.services_config();
    Ok(service(ctx)?
        .refresh(
            owner,
            ctx.app_paths().system().services(),
            &services,
            sources(ctx),
        )
        .await?)
}

pub async fn publish_latest(
    ctx: &AppContext,
    owner: &UserId,
    actor: &UserId,
) -> Result<Vec<LatestPublication>, OrchestrationError> {
    let services = ctx.services_config();
    let service = service(ctx)?;
    let guard = Arc::clone(ctx.kernel().state().get::<Arc<Mutex<PublishGuard>>>()?);
    service
        .refresh(
            owner,
            ctx.app_paths().system().services(),
            &services,
            sources(ctx),
        )
        .await?;
    let mut guard = guard.lock().await;
    Ok(service
        .publish_latest(
            &BaselineScope {
                owner,
                actor,
                root: ctx.app_paths().system().services(),
                services: &services,
                sources: sources(ctx),
            },
            &mut guard,
        )
        .await?)
}
