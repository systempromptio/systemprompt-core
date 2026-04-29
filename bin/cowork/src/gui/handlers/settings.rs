use crate::auth::setup;
use crate::config::paths;
use crate::gui::{GuiApp, window};

pub(crate) fn on_open_settings(app: &mut GuiApp) {
    if let Some(server) = app.ensure_server() {
        let url = server.url();
        app.append_log(format!("opening {url}"));
        window::open_url(&url);
    }
}

pub(crate) fn on_open_config_folder(_app: &mut GuiApp) {
    if let Some(loc) = paths::org_plugins_effective() {
        window::open_path(&loc.path);
    } else if let Ok(s) = setup::status() {
        window::open_path(&s.paths.config_dir);
    }
}
