use winit::event_loop::ActiveEventLoop;

use crate::auth::setup;
use crate::config::paths;
use crate::gui::{GuiApp, window};
use crate::obs::output::diag;

#[tracing::instrument(level = "info", skip(app, event_loop))]
pub(crate) fn on_open_settings(app: &mut GuiApp, event_loop: &ActiveEventLoop) {
    let legacy_origin = app.ensure_server().map(|s| {
        let port = s.port();
        format!("http://127.0.0.1:{port}")
    });
    if let Some(server) = app.server.as_ref() {
        app.append_log(format!("legacy http transport on {}", server.url()));
    }

    if let Some(win) = &app.settings_window {
        win.focus();
        app.append_log("focusing settings window");
        return;
    }

    match window::SettingsWindow::create(event_loop, app.proxy.clone(), legacy_origin) {
        Ok(win) => {
            app.append_log("opened native settings window (sp:// custom protocol)");
            if let Some(handles) = app.menu_bar.as_ref() {
                if let Err(e) = crate::gui::menu::attach_to_window(handles, win.winit_window()) {
                    app.append_log(format!("menu bar attach failed: {e}"));
                }
            }
            app.settings_window = Some(win);
        },
        Err(e) => {
            diag(&format!("gui: native settings window failed: {e}"));
            app.append_log(format!("native window unavailable: {e}"));
        },
    }
}

pub(crate) fn on_open_config_folder(_app: &mut GuiApp) {
    if let Some(loc) = paths::org_plugins_effective() {
        window::open_path(&loc.path);
    } else if let Ok(s) = setup::status() {
        window::open_path(&s.paths.config_dir);
    }
}
