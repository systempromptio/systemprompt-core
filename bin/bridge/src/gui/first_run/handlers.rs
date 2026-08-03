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

pub(crate) fn on_start(app: &mut GuiApp) {
    let hosts: Vec<(String, String)> = crate::integration::host_apps()
        .iter()
        .map(|h| (h.id().to_owned(), h.display_name().to_owned()))
        .collect();
    app.state.begin_first_run(&hosts);
    app.append_log("First use: provisioning your agents…");
    progress(app);
    if hosts.is_empty() {
        advance(app);
        return;
    }
    crate::gui::hosts::tick::request_initial_probe(app);
}

pub(crate) fn on_probe_result(app: &mut GuiApp, host_id: &str, snapshot: &HostAppSnapshot) {
    if app
        .state
        .snapshot()
        .first_run
        .host(host_id)
        .is_none_or(|h| h.status != StepStatus::Probing)
    {
        return;
    }

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
    app.proxy
        .send_event(UiEvent::Host(HostUiEvent::ProfileGenerateRequested {
            host_id: host_id.to_owned(),
            reply_to: None,
        }));
}

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
    app.proxy
        .send_event(UiEvent::Host(HostUiEvent::ProfileInstallRequested {
            host_id: host_id.to_owned(),
            path,
            reply_to: None,
        }));
}

pub(crate) fn on_install_result(app: &mut GuiApp, host_id: &str, error: Option<String>) {
    if let Some(err) = error {
        fail_host(app, host_id, err);
        return;
    }
    app.state
        .set_first_run_host(host_id, StepStatus::Done, None);
    advance(app);
}

pub(crate) fn on_sync_result(app: &mut GuiApp, succeeded: bool) {
    app.state.set_first_run_sync(if succeeded {
        StepStatus::Done
    } else {
        StepStatus::Failed
    });
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

fn advance(app: &mut GuiApp) {
    progress(app);
    if !app.state.snapshot().first_run.all_hosts_terminal() {
        return;
    }
    app.state.set_first_run_phase(FirstRunPhase::Syncing);
    app.state.set_first_run_sync(StepStatus::Installing);
    progress(app);
    app.proxy
        .send_event(UiEvent::SyncRequested { reply_to: None });
}

fn progress(app: &mut GuiApp) {
    emit::emit_first_run_progress(app);
    app.refresh_ui();
}
