use winit::event_loop::ActiveEventLoop;

use crate::auth::setup;
use crate::config::paths;
use crate::gui::{GuiApp, window};
use crate::obs::output::diag;

#[tracing::instrument(level = "debug", skip(app, event_loop))]
pub(crate) fn on_open_settings(app: &mut GuiApp, event_loop: &ActiveEventLoop) {
    let Some(server) = app.ensure_server() else {
        return;
    };
    let (url, port) = (server.url(), server.port());

    if let Some(win) = &app.settings_window {
        win.focus();
        app.append_log(format!("focusing settings window for {url}"));
        return;
    }

    match window::SettingsWindow::create(event_loop, &url, port) {
        Ok(win) => {
            app.append_log(format!("opened native settings window for {url}"));
            app.settings_window = Some(win);
        },
        Err(e) => {
            diag(&format!("gui: native settings window failed: {e}"));
            app.append_log(format!(
                "native window unavailable ({e}); falling back to system browser at {url}"
            ));
            window::open_external_url(&url);
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
