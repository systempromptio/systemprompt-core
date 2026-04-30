use std::path::Path;

use crate::gui::{GuiApp, window};
use crate::integration::find_host_by_id;

pub(crate) fn on_uninstall(app: &mut GuiApp, host_id: &str) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("uninstall: unknown host {host_id}"));
        return;
    };
    app.append_log(format!(
        "uninstall requested for {} (not yet implemented; remove systemprompt-bridge keys \
         manually)",
        host.display_name()
    ));
}

pub(crate) fn on_open_config(app: &mut GuiApp, host_id: &str) {
    let Some(host) = find_host_by_id(host_id) else {
        app.append_log(format!("open-config: unknown host {host_id}"));
        return;
    };
    let snapshot = host.probe();
    if let Some(path) = snapshot.profile_source.as_ref() {
        window::open_path(Path::new(path));
        app.append_log(format!(
            "opened config for {} at {path}",
            host.display_name()
        ));
    } else {
        app.append_log(format!(
            "open-config: no resolved config path for {}",
            host.display_name()
        ));
    }
}

pub(crate) fn on_setup_complete(app: &mut GuiApp) {
    app.state.set_agents_onboarded(true);
    app.append_log("setup marked complete by user");
}
