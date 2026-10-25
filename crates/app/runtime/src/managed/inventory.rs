//! Core inventory orchestration uses configured services roots and
//! organizational ownership.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OrchestrationError;
use crate::AppContext;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::inventory::{
    BaselineScope, InventoryService, InventoryStatus, SourcesContext,
};
pub use systemprompt_marketplace::inventory::{LatestPublication, LatestPublicationStatus};

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
    Ok(
        InventoryService::new(ctx.managed_repository().as_ref().clone())
            .refresh(
                owner,
                ctx.app_paths().system().services(),
                &services,
                sources(ctx),
            )
            .await?,
    )
}

pub async fn publish_latest(
    ctx: &AppContext,
    owner: &UserId,
    actor: &UserId,
) -> Result<Vec<LatestPublication>, OrchestrationError> {
    let services = ctx.services_config();
    let service = InventoryService::new(ctx.managed_repository().as_ref().clone());
    service
        .refresh(
            owner,
            ctx.app_paths().system().services(),
            &services,
            sources(ctx),
        )
        .await?;
    let mut guard = ctx.publish_guard().lock().await;
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
