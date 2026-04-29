use winit::event_loop::ActiveEventLoop;

use crate::auth::setup;
use crate::config::paths;
use crate::gui::{GuiApp, window};
use crate::obs::output::diag;

pub(crate) fn on_open_settings(app: &mut GuiApp, event_loop: &ActiveEventLoop) {
    let (url, port) = match app.ensure_server() {
        Some(server) => (server.url(), server.port()),
        None => return,
    };

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
