//! One-shot boot steps the run loop takes before it serves: registering the
//! document folders as access-control entities and recovering gateway
//! accounting receipts left unsettled by a previous process.
//!
//! Each step is the owning member's work, reached through the contract it
//! provides sealed (`DocumentsKernel` from `documents`,
//! `GatewayOperations` from `gateway`) and called with the kernel's own
//! resolution. A node that does not compose the member has no folders to
//! register or journal to settle, and says so; a member that is composed and
//! fails its step fails the boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use std::sync::Arc;

use systemprompt_extension::router::{DynDocumentsKernel, DynGatewayOperations};
use systemprompt_runtime::AppContext;
use systemprompt_traits::FilesConfigView;

pub(super) async fn reconcile_document_folders(ctx: &AppContext) -> Result<()> {
    if !documents_enabled(ctx) {
        tracing::info!("document store disabled");
        return Ok(());
    }
    let Some(documents) = documents_kernel(ctx) else {
        tracing::info!(
            reason = "documents_member_not_admitted",
            "No document store; no folder entity is registered"
        );
        return Ok(());
    };
    let registered = documents
        .reconcile_folder_entities(&ctx.kernel().resolved())
        .await
        .context("document folder entities")?;
    tracing::debug!(registered, "document folder entities reconciled");
    Ok(())
}

pub(super) async fn start_accounting_recovery(
    ctx: &AppContext,
    gateway: Option<&DynGatewayOperations>,
) -> Result<()> {
    let Some(gateway) = gateway else {
        tracing::info!(
            reason = "gateway_member_not_admitted",
            "No gateway accounting journal to recover"
        );
        return Ok(());
    };
    let settled = gateway
        .recover_accounting(&ctx.kernel().resolved(), ctx.admitted().built())
        .await
        .context("gateway accounting recovery")?;
    if settled > 0 {
        tracing::info!(settled, "Gateway accounting receipts recovered at startup");
    }
    Ok(())
}

pub(super) fn documents_enabled(ctx: &AppContext) -> bool {
    ctx.kernel()
        .state()
        .find::<Arc<FilesConfigView>>()
        .is_some_and(|view| view.documents_enabled)
}

pub(super) fn documents_kernel(ctx: &AppContext) -> Option<DynDocumentsKernel> {
    ctx.kernel().state().find::<DynDocumentsKernel>().cloned()
}
