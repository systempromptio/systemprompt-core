//! Orchestration of the one-time post-link provisioning run.
//!
//! The run does not reimplement probe/generate/install. It drives the existing
//! host handlers by emitting the same [`HostUiEvent`]s the UI does, and is fed
//! back by taps at the end of those handlers. That keeps one code path for
//! "install a host profile" whether a user clicked it or first use did.
//!
//! The first sync runs before any host is touched: the manifest it delivers
//! names the hosts this instance enables, and only those are enrolled or
//! offered a profile. A host the manifest does not enable, or every host when
//! no manifest arrived, is skipped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::HostKind;

use crate::gui::events::UiEvent;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::{GuiApp, emit, first_run};
use crate::integration::enrol::{Outcome, claude_code};
use crate::integration::{AppInstallState, HostAppSnapshot};

use super::state::{FirstRunPhase, StepStatus};

pub(crate) fn on_start(app: &mut GuiApp) {
    let mut hosts: Vec<(HostKind, String)> = crate::integration::host_apps()
        .iter()
        .map(|h| (h.id(), h.display_name().to_owned()))
        .collect();
    hosts.extend(claude_code::installed_agent().map(|a| (a.id, a.display_name.to_owned())));
    app.state.begin_first_run(&hosts);
    app.append_log("First use: syncing your organisation's settings…");
    progress(app);
    start_sync(app);
}

fn provision_enabled_hosts(app: &mut GuiApp, enabled: &[HostKind]) {
    app.state.set_first_run_phase(FirstRunPhase::Probing);
    let pending: Vec<HostKind> = app
        .state
        .snapshot()
        .first_run
        .hosts
        .iter()
        .filter(|h| h.status == StepStatus::Pending)
        .map(|h| h.host_id)
        .collect();
    let mut probe = false;
    for host_id in pending {
        if !enabled.contains(&host_id) {
            app.state.set_first_run_host(
                host_id,
                StepStatus::Skipped,
                Some("not enabled on this instance".to_owned()),
            );
            app.append_log(format!(
                "[{host_id}] not enabled on this instance — skipped"
            ));
        } else if host_id == HostKind::ClaudeCode {
            enrol_claude_code(app);
        } else {
            app.state
                .set_first_run_host(host_id, StepStatus::Probing, None);
            probe = true;
        }
    }
    progress(app);
    if probe {
        crate::gui::hosts::tick::request_initial_probe(app);
    }
}

// Why: with no delivered manifest the enabled hosts are unknown, so none is
// provisioned rather than all of them.
fn delivered_enabled_hosts(app: &GuiApp) -> Vec<HostKind> {
    match crate::sync::last_synced_enabled_hosts() {
        Ok(Some(hosts)) => hosts,
        Ok(None) => Vec::new(),
        Err(e) => {
            app.append_log_error(format!(
                "First use: the synced host list could not be read ({e}); no agent is set up"
            ));
            Vec::new()
        },
    }
}

fn enrol_claude_code(app: &GuiApp) {
    let report = claude_code::enrol_report(&app.ctx);
    let id = report.host_id;
    if let Outcome::Failed(err) = report.outcome {
        app.append_log_error(format!("[{id}] gateway routing not written: {err}"));
        app.state
            .set_first_run_host(id, StepStatus::Failed, Some(err.to_string()));
    } else {
        app.append_log(format!("[{id}] {}", report.install_action_label));
        app.state.set_first_run_host(id, StepStatus::Done, None);
    }
}

pub(crate) fn on_probe_result(app: &mut GuiApp, host_id: HostKind, snapshot: &HostAppSnapshot) {
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
            host_id,
            reply_to: None,
        }));
}

pub(crate) fn on_generate_result(app: &mut GuiApp, host_id: HostKind, error: Option<String>) {
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
            host_id,
            path,
            reply_to: None,
        }));
}

pub(crate) fn on_install_result(app: &mut GuiApp, host_id: HostKind, error: Option<String>) {
    if let Some(err) = error {
        fail_host(app, host_id, err);
        return;
    }
    app.state
        .set_first_run_host(host_id, StepStatus::Done, None);
    app.state.update_first_run(|s| {
        if s.sync != StepStatus::Pending {
            s.host_installed_during_sync = true;
        }
    });
    advance(app);
}

pub(crate) fn on_sync_cancelled(app: &mut GuiApp) {
    app.state.set_first_run_sync(StepStatus::Pending);
    app.append_log("First use: sync cancelled — retry when you are ready.");
    progress(app);
    emit::emit_state(app);
}

pub(crate) fn on_sync_result(app: &mut GuiApp, succeeded: bool) {
    let awaiting_hosts = app
        .state
        .snapshot()
        .first_run
        .hosts
        .iter()
        .any(|h| h.status == StepStatus::Pending);
    app.state.update_first_run(|s| {
        let first = s.sync_result.unwrap_or(true);
        s.sync_result = Some(if s.followup_sync {
            first && succeeded
        } else {
            succeeded
        });
        s.followup_sync = false;
        s.sync = if succeeded {
            StepStatus::Done
        } else {
            StepStatus::Failed
        };
    });
    if awaiting_hosts {
        let enabled = if succeeded {
            delivered_enabled_hosts(app)
        } else {
            Vec::new()
        };
        provision_enabled_hosts(app, &enabled);
    }
    try_finish(app);
}

// Why: Finalises the run once both the sync result and every host are in,
// whichever arrives last. A host that installed while the first sync ran
// is owed one follow-up sync so its settings are applied.
fn try_finish(app: &mut GuiApp) {
    let state = app.state.snapshot().first_run;
    if state.sync_result.is_none() || !state.all_hosts_terminal() || state.followup_sync {
        if state.all_hosts_terminal() {
            app.state.set_first_run_phase(FirstRunPhase::Syncing);
        }
        progress(app);
        emit::emit_state(app);
        return;
    }
    if state.host_installed_during_sync && state.sync_result == Some(true) {
        app.state.update_first_run(|s| {
            s.host_installed_during_sync = false;
            s.followup_sync = true;
        });
        app.append_log("First use: applying settings to newly installed agents…");
        start_sync(app);
        return;
    }
    finish(app, state.sync_result == Some(true));
}

pub(crate) fn finish(app: &mut GuiApp, succeeded: bool) {
    let state = app.state.snapshot().first_run;
    let usable = succeeded || state.any_host_installed();
    app.state.finish_first_run(if usable {
        FirstRunPhase::Complete
    } else {
        FirstRunPhase::Failed
    });
    let state = app.state.snapshot().first_run;
    first_run::record::write(&state);
    if usable {
        app.append_log("First use: setup complete.");
    } else {
        app.append_log_error("First use: setup failed — see the errors above, then retry.");
    }
    progress(app);
    emit::emit_state(app);
}

fn fail_host(app: &mut GuiApp, host_id: HostKind, error: String) {
    app.state
        .set_first_run_host(host_id, StepStatus::Failed, Some(error));
    advance(app);
}

fn advance(app: &mut GuiApp) {
    try_finish(app);
}

fn start_sync(app: &mut GuiApp) {
    if app.state.snapshot().first_run.all_hosts_terminal() {
        app.state.set_first_run_phase(FirstRunPhase::Syncing);
    }
    app.state.set_first_run_sync(StepStatus::Installing);
    progress(app);
    app.did_initial_sync = true;
    app.proxy
        .send_event(UiEvent::SyncRequested { reply_to: None });
}

fn progress(app: &mut GuiApp) {
    emit::emit_first_run_progress(app);
    app.refresh_ui();
}
