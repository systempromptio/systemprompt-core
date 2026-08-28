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

const TIMEOUT_SECS: u64 = 300;

// Why: closing hides to the tray. On Windows the difference between "minimised"
// and "a zombie in Task Manager" is entirely whether anyone said so, and it
// only needs saying once.
pub(crate) fn notify_closed_to_tray() {
    if record::tray_notice_shown() {
        return;
    }
    record::mark_tray_notice_shown();
    let app = crate::brand::brand().app_name;
    crate::gui::window::notify_user(
        &format!("{app} is still running"),
        "It keeps governing your agents from the notification area. Quit it from the tray menu to \
         stop.",
    );
}

pub(crate) fn should_run(app: &crate::gui::GuiApp) -> bool {
    record::read().is_none() && !app.state.snapshot().first_run.active
}

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
    app.append_log_warn("First use: setup timed out; continuing with what succeeded.");
    handlers::on_sync_result(app, false);
}
