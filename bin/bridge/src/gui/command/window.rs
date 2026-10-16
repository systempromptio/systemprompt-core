//! Window chrome command dispatch: dragging and zooming from the webview.
//!
//! On macOS the webview covers the transparent titlebar, so the page's own
//! topbar is the only surface the user can grab; it asks for the move here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;

use super::CommandOutcome;
use crate::gui::GuiApp;

pub(super) fn window_dispatch(app: &GuiApp, cmd: &str) -> Option<CommandOutcome> {
    let win = app.settings_window.as_ref();
    match cmd {
        "window.drag" => {
            if let Some(win) = win {
                win.drag();
            }
        },
        "window.toggleMaximize" => {
            if let Some(win) = win {
                win.toggle_maximize();
            }
        },
        _ => return None,
    }
    Some(CommandOutcome::Sync(Ok(json!({}))))
}
