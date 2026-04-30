use crate::gui::GuiApp;
use crate::gui::events::UiEvent;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::state::now_unix;

pub(crate) fn maybe_probe(app: &GuiApp) {
    let snap = app.state.snapshot();
    for host in crate::integration::host_apps() {
        let id = host.id();
        let st = snap.hosts.get(id);
        if let Some(state) = st {
            if state.probe_in_flight {
                continue;
            }
            let due = state
                .snapshot
                .as_ref()
                .map(|s| {
                    now_unix().saturating_sub(s.probed_at_unix) >= super::super::PROBE_INTERVAL_SECS
                })
                .unwrap_or(true);
            if !due {
                continue;
            }
        }
        let _ = app
            .proxy
            .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
                host_id: id.to_string(),
                reply_to: None,
            }));
    }
    if !snap.hosts.proxy_probe_in_flight {
        let due = now_unix().saturating_sub(snap.hosts.local_proxy.probed_at_unix)
            >= super::super::PROBE_INTERVAL_SECS;
        if due {
            let _ = app
                .proxy
                .send_event(UiEvent::Host(HostUiEvent::ProxyProbeRequested {
                    reply_to: None,
                }));
        }
    }
}

pub(crate) fn request_initial_probe(app: &GuiApp) {
    for host in crate::integration::host_apps() {
        let _ = app
            .proxy
            .send_event(UiEvent::Host(HostUiEvent::ProbeRequested {
                host_id: host.id().to_string(),
                reply_to: None,
            }));
    }
}
