use crate::gui::GuiApp;
use crate::gui::claude::events::ClaudeUiEvent;
use crate::gui::claude::handlers;

pub(crate) fn handle(app: &mut GuiApp, event: ClaudeUiEvent) {
    match event {
        ClaudeUiEvent::ProbeRequested => handlers::on_probe_requested(app),
        ClaudeUiEvent::ProbeFinished(s) => handlers::on_probe_finished(app, s),
        ClaudeUiEvent::ProfileGenerateRequested => handlers::on_profile_generate_requested(app),
        ClaudeUiEvent::ProfileGenerateFinished(r) => handlers::on_profile_generate_finished(app, r),
        ClaudeUiEvent::ProfileInstallRequested(p) => handlers::on_profile_install_requested(app, p),
        ClaudeUiEvent::ProfileInstallFinished(r) => handlers::on_profile_install_finished(app, r),
    }
}
