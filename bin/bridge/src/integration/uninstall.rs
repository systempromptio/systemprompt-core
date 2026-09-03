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

    // Why: the plugin dirs are gone by now, but `~/.claude` still enables them
    // and still carries their `hooks.json` — hooks that would keep firing at a
    // loopback port this uninstall guarantees will never come up again.
    if let Err(e) = super::claude_code_cli::clear_install() {
        diag(&format!("warning: Claude Code CLI cleanup failed: {e}"));
    }
}

#[derive(Debug)]
pub struct PurgeReport {
    pub uninstall: crate::install::UninstallSummary,
    pub clean: crate::auth::setup::CleanReport,
}

// Why: `uninstall --purge` leaves the config file and the onboarding sentinels
// behind, so the next launch still knows the gateway and skips the wizard. The
// GUI's "remove everything" promises a machine that has never seen the bridge,
// which is the union of uninstall, the host cleanup and `clean`.
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
