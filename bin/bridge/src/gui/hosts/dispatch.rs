use crate::gui::GuiApp;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::hosts::handlers;

pub(crate) fn handle(app: &mut GuiApp, event: HostUiEvent) {
    match event {
        HostUiEvent::ProbeRequested { host_id, reply_to } => {
            handlers::on_probe_requested(app, &host_id, reply_to)
        },
        HostUiEvent::ProbeFinished {
            host_id,
            snapshot,
            reply_to,
        } => handlers::on_probe_finished(app, &host_id, *snapshot, reply_to),
        HostUiEvent::ProfileGenerateRequested { host_id, reply_to } => {
            handlers::on_profile_generate_requested(app, &host_id, reply_to)
        },
        HostUiEvent::ProfileGenerateFinished {
            host_id,
            result,
            reply_to,
        } => handlers::on_profile_generate_finished(app, &host_id, result, reply_to),
        HostUiEvent::ProfileInstallRequested {
            host_id,
            path,
            reply_to,
        } => handlers::on_profile_install_requested(app, &host_id, path, reply_to),
        HostUiEvent::ProfileInstallFinished {
            host_id,
            result,
            reply_to,
        } => handlers::on_profile_install_finished(app, &host_id, result, reply_to),
        HostUiEvent::ProxyProbeRequested { reply_to } => {
            handlers::on_proxy_probe_requested(app, reply_to)
        },
        HostUiEvent::ProxyProbeFinished { health, reply_to } => {
            handlers::on_proxy_probe_finished(app, *health, reply_to)
        },
    }
}
