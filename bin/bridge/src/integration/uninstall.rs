//! What the hosts leave behind when the bridge is uninstalled.
//!
//! Cowork's enable keys and Claude Code's plugin registrations. Called from
//! the uninstall command after `install::uninstall` has removed the bridge's
//! own files; it lives here, not in `install`, so `install` never names a host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::stdio::diag;

pub fn clear_hosts() -> Vec<String> {
    let mut warnings = Vec::new();
    if let Some(target) = super::cowork_plugins::resolve_target()
        && let Err(e) = super::cowork_plugins::clear_all(&target)
    {
        warnings.push(format!("Cowork enable-key cleanup failed: {e}"));
    }

    if let Err(e) = super::claude_code_cli::clear_install() {
        warnings.push(format!("Claude Code CLI cleanup failed: {e}"));
    }

    // Why: a profile left on any host keeps pointing that host at a proxy port
    // and secret the purge has just discarded; every enrolled host is cleared,
    // not only the two the Cowork path knew about.
    match super::enrol::remove_host_profiles(&super::enrol::Selection::All) {
        Ok(reports) => {
            for report in reports {
                match report.outcome {
                    super::enrol::Outcome::Failed(e) => {
                        warnings.push(format!("{}: profile removal failed: {e}", report.host_id));
                    },
                    super::enrol::Outcome::Declined => {
                        warnings.push(format!(
                            "{}: administrator approval declined; its profile is still installed",
                            report.host_id
                        ));
                    },
                    super::enrol::Outcome::ManualStep(instruction) => {
                        warnings.push(format!("{}: {instruction}", report.host_id));
                    },
                    _ => {},
                }
            }
        },
        Err(e) => warnings.push(format!("host profile removal failed: {e}")),
    }
    for warning in &warnings {
        diag(&format!("warning: {warning}"));
    }
    warnings
}

#[derive(Debug)]
pub struct PurgeReport {
    pub uninstall: crate::install::UninstallSummary,
    pub clean: crate::auth::setup::CleanReport,
    pub warnings: Vec<String>,
    pub foreign_proxy: Option<String>,
    pub proxy_state_removed: Vec<std::path::PathBuf>,
}

impl PurgeReport {
    #[must_use]
    pub fn leftovers(&self) -> Vec<String> {
        let mut out = self.warnings.clone();
        if let crate::install::ManagedProfileOutcome::RemoveFailed(e) =
            &self.uninstall.managed_profile
        {
            out.push(format!("managed Claude policy: {e}"));
        }
        if let Some(who) = &self.foreign_proxy {
            out.push(format!(
                "another account on this computer is running the bridge from {who}; it was not \
                 touched, and it keeps port {} until it is quit",
                crate::proxy::DEFAULT_PROXY_PORT
            ));
        }
        out
    }
}

#[tracing::instrument(level = "info", skip(ctx))]
pub fn purge_device(
    ctx: &crate::context::BridgeContext,
) -> Result<PurgeReport, crate::install::InstallError> {
    let uninstall = crate::install::uninstall(true, ctx)?;
    let warnings = clear_hosts();
    let clean = crate::auth::setup::clean()
        .map_err(|e| crate::install::InstallError::Bootstrap(format!("clean local state: {e}")))?;
    let proxy_state_removed = remove_proxy_state()?;
    let foreign_proxy = match crate::proxy::peer::probe_identity(
        crate::proxy::DEFAULT_PROXY_PORT,
        ctx.install_id(),
    ) {
        crate::proxy::peer::PeerIdentity::Foreign(who) => Some(who.config_dir),
        _ => None,
    };
    Ok(PurgeReport {
        uninstall,
        clean,
        warnings,
        foreign_proxy,
        proxy_state_removed,
    })
}

// Why: a purge that keeps the loopback key keeps whatever is wrong with it; an
// unreadable or foreign-owned key survived every "remove everything" and the
// next start failed the same way. Fresh state means a fresh secret, install
// identity and port record, minted by the next start.
pub fn remove_proxy_state() -> Result<Vec<std::path::PathBuf>, crate::install::InstallError> {
    let mut removed = Vec::new();
    let candidates = [
        crate::proxy::secret::secret_path(),
        crate::proxy::identity::install_id_path(),
        crate::proxy::portfile::portfile_path(),
    ];
    for path in candidates.into_iter().flatten() {
        if !path.exists() {
            continue;
        }
        crate::fsutil::remove_verified(&path).map_err(|e| {
            crate::install::InstallError::Bootstrap(format!("remove {}: {e}", path.display()))
        })?;
        removed.push(path);
    }
    Ok(removed)
}
