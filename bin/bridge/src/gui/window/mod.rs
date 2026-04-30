pub mod native;

use std::path::Path;
use std::process::Command;

pub use native::SettingsWindow;

pub fn open_path(path: &Path) {
    open_target(&path.to_string_lossy());
}

pub fn open_external_url(url: &str) {
    open_target(url);
}

fn open_target(target: &str) {
    let program = std::cfg_select! {
        target_os = "macos"   => "/usr/bin/open",
        target_os = "windows" => "cmd",
        _                     => "xdg-open",
    };
    let prefix: &[&str] = std::cfg_select! {
        target_os = "windows" => &["/C", "start", ""],
        _                     => &[],
    };
    tracing::info!(target = %target, program, "opening external target");
    match Command::new(program).args(prefix).arg(target).spawn() {
        Ok(_) => {},
        Err(e) => tracing::error!(target = %target, program, error = %e, "failed to spawn opener"),
    }
}
