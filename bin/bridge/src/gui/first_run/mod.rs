//! One-time provisioning after the first device link.
//!
//! Linking a device used to leave the app unusable until the user found the
//! agents tab and ran the install by hand. This module runs that install
//! automatically the first time a device is linked — probe every registered
//! host, generate and install its profile, then sync — and reports progress
//! into the setup wizard so a failure is visible rather than silent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod handlers;
pub mod record;
pub(crate) mod serde;
pub mod state;

/// How long the whole run may take before the watchdog ends it.
const TIMEOUT_SECS: u64 = 300;

/// Whether first-use provisioning should run now.
///
/// False once the sentinel exists, so signing out and back in does not
/// re-provision, and false while a run is already in flight.
pub(crate) fn should_run(app: &crate::gui::GuiApp) -> bool {
    record::read().is_none() && !app.state.snapshot().first_run.active
}

/// End a run that has stopped making progress.
///
/// Every stage advances on an event, and a probe or install that never reports
/// back (a panicked blocking task, a UAC prompt left unanswered) would leave
/// the run active forever — with the wizard's Finish button disabled, that
/// locks the user out of the app entirely. Being stuck outside is worse than
/// the half-installed state this flow exists to prevent, so time out and let
/// them in with the failures on screen.
pub(crate) fn tick(app: &mut crate::gui::GuiApp) {
    let state = app.state.snapshot().first_run;
    if !state.active {
        return;
    }
    let elapsed = crate::gui::state::now_unix().saturating_sub(state.started_at_unix);
    if elapsed < TIMEOUT_SECS {
        return;
    }
    for host in &state.hosts {
        if !host.status.is_terminal() {
            app.state.set_first_run_host(
                &host.host_id,
                state::StepStatus::Failed,
                Some(format!("timed out after {TIMEOUT_SECS}s")),
            );
        }
    }
    app.append_log("First use: setup timed out; continuing with what succeeded.");
    handlers::on_sync_result(app, false);
}
