//! Carrying a pending administrator approval across the update restart and
//! applying it once the successor has synced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::gui::GuiApp;

const APPROVE_AFTER_UPDATE: &str = "approve-after-update.json";

fn approval_marker() -> Option<std::path::PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|d| d.join(APPROVE_AFTER_UPDATE))
}

// Why: a machine-policy change waiting on approval is rendered again by the
// updated binary; carrying the request across the restart lets the new
// process ask once, instead of once before and once after.
pub(super) fn remember_pending_approval(gate: &crate::elevation_gate::ElevationGate) {
    if gate.pending().is_none() {
        return;
    }
    if let Some(path) = approval_marker()
        && let Err(error) = crate::fsutil::atomic_write_0600(&path, b"{}")
    {
        tracing::warn!(%error, "update: the pending administrator approval was not carried over");
    }
}

pub(crate) fn approve_after_update(app: &mut GuiApp) {
    if app.approval_after_update_checked {
        return;
    }
    let snap = app.state.snapshot();
    if !app.did_initial_sync || snap.sync_in_flight {
        return;
    }
    app.approval_after_update_checked = true;
    let Some(path) = approval_marker().filter(|p| p.exists()) else {
        return;
    };
    if let Err(error) = crate::fsutil::remove_verified(&path) {
        tracing::warn!(%error, "update: approval marker could not be removed");
        return;
    }
    app.append_log("update: applying the policy change approved with the restart");
    let ctx = Arc::clone(&app.ctx);
    app.ctx.spawn(async move {
        let overrides = crate::integration::reapply::ModelProtocolOverrides::new();
        let reports = crate::integration::reapply::reapply_stale_profiles(
            &ctx,
            &overrides,
            crate::integration::reapply::Attendance::Attended,
        )
        .await;
        tracing::info!(
            report = %crate::integration::reapply::render(&reports),
            "post-update policy approval"
        );
    });
}
