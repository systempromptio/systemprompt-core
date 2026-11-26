//! One-shot boot steps the run loop takes before it serves: registering the
//! document folders as access-control entities and recovering gateway
//! accounting receipts left unsettled by a previous process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;

use systemprompt_runtime::AppContext;
use systemprompt_traits::{BackgroundTasks, FilesConfigView};

pub(super) async fn reconcile_document_folders(ctx: &AppContext) -> Result<()> {
    if !documents_enabled(ctx) {
        tracing::info!("document store disabled");
        return Ok(());
    }
    let registered = systemprompt_documents::bound_handles(ctx.kernel().state())
        .context("document store")?
        .access
        .reconcile_folder_entities()
        .await
        .context("document folder entities")?;
    tracing::debug!(registered, "document folder entities reconciled");
    Ok(())
}

pub(super) async fn start_accounting_recovery(ctx: &AppContext) -> Result<()> {
    if !systemprompt_gateway::audit::journal::recovery_admitted(ctx.admitted().built()) {
        tracing::debug!("ai member not admitted; gateway accounting recovery skipped");
        return Ok(());
    }
    let settlement = crate::routes::gateway::gateway_repositories(ctx)?.settlement();
    let settled = systemprompt_gateway::audit::journal::recover(&settlement).await?;
    if settled > 0 {
        tracing::info!(settled, "Gateway accounting receipts recovered at startup");
    }
    systemprompt_gateway::audit::journal::spawn_recovery(
        settlement,
        ctx.kernel().state().get::<BackgroundTasks>()?,
    );
    Ok(())
}

pub(super) fn documents_enabled(ctx: &AppContext) -> bool {
    ctx.kernel()
        .state()
        .find::<Arc<FilesConfigView>>()
        .is_some_and(|view| view.documents_enabled)
}
