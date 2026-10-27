//! One-shot boot steps the run loop takes before it serves: registering the
//! document folders as access-control entities, and recovering gateway
//! accounting receipts left unsettled by a previous process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use systemprompt_runtime::AppContext;

pub(super) async fn reconcile_document_folders(ctx: &AppContext) -> Result<()> {
    let registered = ctx
        .documents()
        .reconcile_folder_entities()
        .await
        .context("document folder entities")?;
    tracing::debug!(registered, "document folder entities reconciled");
    Ok(())
}

pub(super) async fn start_accounting_recovery(ctx: &AppContext) -> Result<()> {
    let settlement = crate::routes::gateway::gateway_repositories(ctx)?.settlement();
    let settled = systemprompt_gateway::audit::journal::recover(&settlement).await?;
    if settled > 0 {
        tracing::info!(settled, "Gateway accounting receipts recovered at startup");
    }
    systemprompt_gateway::audit::journal::spawn_recovery(settlement, ctx.background_tasks());
    Ok(())
}
