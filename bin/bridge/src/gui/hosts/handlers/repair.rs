//! The repair the bridge starts on its own: a profile it installed that a
//! later release reads as stale is re-rendered at launch, without a prompt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::gui::hosts::events::{HostUiEvent, ProbeCause};
use crate::ids::HostId;
use crate::integration::HostAppSnapshot;
use crate::integration::profile_state::{ProfileState, StaleReason};
use crate::integration::reapply::{Attendance, Outcome, Report};

pub(crate) fn repair_stale_unattended(
    app: &mut GuiApp,
    host_id: &HostId,
    snapshot: &HostAppSnapshot,
) {
    let ProfileState::Stale { reason } = &snapshot.profile_state else {
        return;
    };
    let state = app.state.snapshot();
    if !state.signed_in() || state.first_run.active {
        return;
    }
    if !app.unattended_repairs.insert(host_id.clone()) {
        return;
    }
    let Some(host) = crate::integration::find_host_by_id(host_id.as_str()) else {
        return;
    };
    let why = match reason {
        StaleReason::LoopbackSecret => "its credential is from an earlier release",
        StaleReason::ProxyPort => "the proxy port moved",
    };
    app.append_log(format!(
        "[{host_id}] configuration profile is out of date — {why}; re-applying it"
    ));
    let overrides = state.host_model_protocols;
    let env = app.probe_env();
    let proxy = app.proxy.clone();
    let bridge = Arc::clone(&app.ctx);
    let host_id_owned = host_id.clone();
    app.ctx.spawn(async move {
        let report = crate::integration::reapply::reapply_host(
            &bridge,
            host,
            &overrides,
            &env,
            Attendance::Unattended,
        )
        .await;
        proxy.send_event(UiEvent::Host(HostUiEvent::UnattendedRepairFinished {
            host_id: host_id_owned,
            report,
        }));
    });
}

pub(crate) fn on_unattended_repair_finished(app: &GuiApp, host_id: &HostId, report: &Report) {
    match &report.outcome {
        Outcome::Reapplied => {
            app.append_log(format!("[{host_id}] configuration profile refreshed"));
        },
        Outcome::Pending => app.append_log(format!(
            "[{host_id}] configuration profile handed to the OS; approve it to finish ({})",
            report.install_action_label
        )),
        Outcome::Declined => app.append_log_warn(format!(
            "[{host_id}] configuration profile needs your approval to refresh — use Repair in \
             the Agents tab"
        )),
        Outcome::Failed(e) => {
            app.append_log_error(format!(
                "[{host_id}] configuration profile refresh failed: {e}"
            ));
        },
    }
    for warning in &report.warnings {
        app.append_log_warn(format!("[{host_id}] {warning}"));
    }
    app.proxy
        .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
            host_id: host_id.clone(),
            cause: ProbeCause::Manual,
            reply_to: None,
        }));
}
