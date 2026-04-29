use crate::gui::GuiApp;
use crate::gui::hosts::events::HostUiEvent;
use crate::gui::hosts::handlers;

pub(crate) fn handle(app: &mut GuiApp, event: HostUiEvent) {
    match event {
        HostUiEvent::ProbeRequested { host_id } => handlers::on_probe_requested(app, &host_id),
        HostUiEvent::ProbeFinished { host_id, snapshot } => {
            handlers::on_probe_finished(app, &host_id, *snapshot)
        },
        HostUiEvent::ProfileGenerateRequested { host_id } => {
            handlers::on_profile_generate_requested(app, &host_id)
        },
        HostUiEvent::ProfileGenerateFinished { host_id, result } => {
            handlers::on_profile_generate_finished(app, &host_id, result)
        },
        HostUiEvent::ProfileInstallRequested { host_id, path } => {
            handlers::on_profile_install_requested(app, &host_id, path)
        },
        HostUiEvent::ProfileInstallFinished { host_id, result } => {
            handlers::on_profile_install_finished(app, &host_id, result)
        },
        HostUiEvent::ProxyProbeRequested => handlers::on_proxy_probe_requested(app),
        HostUiEvent::ProxyProbeFinished(health) => handlers::on_proxy_probe_finished(app, *health),
    }
}
