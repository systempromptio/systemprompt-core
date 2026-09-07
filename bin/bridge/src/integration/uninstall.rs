//! What the hosts leave behind when the bridge is uninstalled.
//!
//! Cowork's enable keys and Claude Code's plugin registrations. Called from
//! the uninstall command after `install::uninstall` has removed the bridge's
//! own files; it lives here, not in `install`, so `install` never names a host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::stdio::diag;

pub fn clear_hosts() {
    if let Some(target) = super::cowork_plugins::resolve_target()
        && let Err(e) = super::cowork_plugins::clear_all(&target)
    {
        diag(&format!("warning: Cowork enable-key cleanup failed: {e}"));
    }

    if let Err(e) = super::claude_code_cli::clear_install() {
        diag(&format!("warning: Claude Code CLI cleanup failed: {e}"));
    }
}

#[derive(Debug)]
pub struct PurgeReport {
    pub uninstall: crate::install::UninstallSummary,
    pub clean: crate::auth::setup::CleanReport,
}

#[tracing::instrument(level = "info", skip(ctx))]
pub fn purge_device(
    ctx: &crate::context::BridgeContext,
) -> Result<PurgeReport, crate::install::InstallError> {
    let uninstall = crate::install::uninstall(true, ctx)?;
    clear_hosts();
    let clean = crate::auth::setup::clean()
        .map_err(|e| crate::install::InstallError::Bootstrap(format!("clean local state: {e}")))?;
    Ok(PurgeReport { uninstall, clean })
}
