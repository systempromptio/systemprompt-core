use crate::gui::GuiApp;
use crate::gui::claude::events::ClaudeUiEvent;
use crate::gui::events::UiEvent;
use crate::gui::state::now_unix;

pub(crate) fn maybe_probe(app: &GuiApp) {
    let snap = app.state.snapshot();
    let claude_due = snap
        .claude
        .integration
        .as_ref()
        .map(|c| now_unix().saturating_sub(c.probed_at_unix) >= super::super::PROBE_INTERVAL_SECS)
        .unwrap_or(true);
    if claude_due && !snap.claude.probe_in_flight {
        let _ = app
            .proxy
            .send_event(UiEvent::Claude(ClaudeUiEvent::ProbeRequested));
    }
}

pub(crate) fn request_initial_probe(app: &GuiApp) {
    let _ = app
        .proxy
        .send_event(UiEvent::Claude(ClaudeUiEvent::ProbeRequested));
}
