use crate::gui::GuiApp;

pub(crate) fn on_state_refreshed(app: &mut GuiApp) {
    app.state.reload();
    app.refresh_ui();
}
