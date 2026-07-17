//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gui::GuiApp;

pub(crate) fn on_state_refreshed(app: &mut GuiApp) {
    app.state.reload();
    app.refresh_ui();
}
