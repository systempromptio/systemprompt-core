//! Orchestration of the one-time post-link provisioning run.
//!
//! The run does not reimplement probe/generate/install. It drives the existing
//! host handlers by emitting the same [`HostUiEvent`]s the UI does, and is fed
//! back by taps at the end of those handlers. That keeps one code path for
//! "install a host profile" whether a user clicked it or first use did.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::events::UiEvent;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::{GuiApp, emit, first_run};
use crate::integration::{AppInstallState, HostAppSnapshot};

use super::state::{FirstRunPhase, StepStatus};

/// Begin the run: seed one row per registered host and kick off the probes.
pub(crate) fn on_start(app: &mut GuiApp) {
    let hosts: Vec<(String, String)> = crate::integration::host_apps()
        .iter()
        .map(|h| (h.id().to_owned(), h.display_name().to_owned()))
        .collect();
    app.state.begin_first_run(&hosts);
    app.append_log("First use: provisioning your agents…");
    progress(app);
    if hosts.is_empty() {
        // Nothing to probe means nothing will ever call `advance`, so go
        // straight to the sync rather than waiting out the watchdog.
        advance(app);
        return;
    }
    // Reuse the normal probe path; results come back through the tap in
    // `hosts::handlers::on_probe_finished`.
    crate::gui::hosts::tick::request_initial_probe(app);
}

/// A probe landed. Decide whether this host gets a profile.
pub(crate) fn on_probe_result(app: &mut GuiApp, host_id: &str, snapshot: &HostAppSnapshot) {
    // Only a host still waiting on its first probe advances. Terminal hosts are
    // done, and the periodic tick probe (or the re-probe fired by
    // `on_profile_install_finished`) would otherwise restart a chain that is
    // already generating or installing.
    if app
        .state
        .snapshot()
        .first_run
        .host(host_id)
        .is_none_or(|h| h.status != StepStatus::Probing)
    {
        return;
    }

    // `Unknown` attempts the install anyway. Some hosts cannot report their
    // install state on every platform, and skipping those is exactly the
    // half-provisioned state this flow exists to prevent.
    if snapshot.app_installed == AppInstallState::NotInstalled {
        app.state
            .set_first_run_host(host_id, StepStatus::Skipped, None);
        app.append_log(format!(
            "[{host_id}] not installed on this machine — skipped"
        ));
        advance(app);
        return;
    }

    app.state
        .set_first_run_host(host_id, StepStatus::Generating, None);
    app.state.set_first_run_phase(FirstRunPhase::Installing);
    progress(app);
    _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProfileGenerateRequested {
            host_id: host_id.to_owned(),
            reply_to: None,
        }));
}

/// Profile generation finished. On success chain straight into the install.
pub(crate) fn on_generate_result(app: &mut GuiApp, host_id: &str, error: Option<String>) {
    if let Some(err) = error {
        fail_host(app, host_id, err);
        return;
    }
    let path = app
        .state
        .snapshot()
        .hosts
        .get(host_id)
        .and_then(|h| h.last_generated_profile.as_ref().map(|p| p.path.clone()));
    let Some(path) = path else {
        fail_host(
            app,
            host_id,
            "profile was generated but its path was not recorded".to_owned(),
        );
        return;
    };
    app.state
        .set_first_run_host(host_id, StepStatus::Installing, None);
    progress(app);
    _ = app
        .proxy
        .send_event(UiEvent::Host(HostUiEvent::ProfileInstallRequested {
            host_id: host_id.to_owned(),
            path,
            reply_to: None,
        }));
}

/// Install finished. One host failing never stops the others.
pub(crate) fn on_install_result(app: &mut GuiApp, host_id: &str, error: Option<String>) {
    if let Some(err) = error {
        fail_host(app, host_id, err);
        return;
    }
    app.state
        .set_first_run_host(host_id, StepStatus::Done, None);
    advance(app);
}

/// The trailing sync finished — the run is over either way.
pub(crate) fn on_sync_result(app: &mut GuiApp, succeeded: bool) {
    app.state.set_first_run_sync(if succeeded {
        StepStatus::Done
    } else {
        StepStatus::Failed
    });
    // Failed only when nothing at all was provisioned. A sync error with hosts
    // installed still leaves a working app, and the next scheduled sync retries.
    let state = app.state.snapshot().first_run;
    let usable = succeeded || state.any_host_installed();
    app.state.finish_first_run(if usable {
        FirstRunPhase::Complete
    } else {
        FirstRunPhase::Failed
    });
    let state = app.state.snapshot().first_run;
    first_run::record::write(&state);
    app.append_log(if usable {
        "First use: setup complete."
    } else {
        "First use: setup failed — see the errors above, then retry."
    });
    progress(app);
    emit::emit_state(app);
}

fn fail_host(app: &mut GuiApp, host_id: &str, error: String) {
    app.state
        .set_first_run_host(host_id, StepStatus::Failed, Some(error));
    advance(app);
}

/// Move to the sync stage once every host has settled.
fn advance(app: &mut GuiApp) {
    progress(app);
    if !app.state.snapshot().first_run.all_hosts_terminal() {
        return;
    }
    app.state.set_first_run_phase(FirstRunPhase::Syncing);
    app.state.set_first_run_sync(StepStatus::Installing);
    progress(app);
    _ = app
        .proxy
        .send_event(UiEvent::SyncRequested { reply_to: None });
}

fn progress(app: &mut GuiApp) {
    emit::emit_first_run_progress(app);
    app.refresh_ui();
}
