//! Webview window management and external-target opening.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod native;
mod native_protocol;

pub use crate::window_state as geometry;

#[cfg(target_os = "windows")]
mod dwm;
#[cfg(target_os = "windows")]
mod msgbox;

use std::path::Path;
use std::process::Command;

pub use native::SettingsWindow;

#[cfg(target_os = "windows")]
pub use dwm::set_immersive_dark;

#[cfg(not(target_os = "windows"))]
pub const fn set_immersive_dark(_window: &dyn winit::window::Window, _dark: bool) {}

#[must_use]
pub fn prefers_dark(window: &dyn winit::window::Window) -> bool {
    // Why: winit returns None on X11 and Android, and a platform that will not
    // say gets the app's own default, which is dark.
    !matches!(window.theme(), Some(winit::window::Theme::Light))
}

pub fn open_path(path: &Path) {
    open_target(&path.to_string_lossy());
}

pub fn open_external_url(url: &str) {
    open_target(url);
}

pub fn alert_user(title: &str, message: &str) {
    tracing::warn!(title = %title, message = %message, "alerting user");
    #[cfg(target_os = "windows")]
    {
        msgbox::show(title, message);
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Why: the shells below are quote-delimited; dropping quotes from the
        // interpolated text keeps the command well-formed without an escaper.
        let title = title.replace(['"', '\''], "");
        let message = message.replace(['"', '\''], "");
        let spawned = std::cfg_select! {
            target_os = "macos" => Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(format!(
                    "display dialog \"{message}\" with title \"{title}\" buttons {{\"OK\"}} with icon stop"
                ))
                .status(),
            _ => Command::new("notify-send")
                .args(["--urgency=critical", &title, &message])
                .status(),
        };
        if let Err(e) = spawned {
            tracing::error!(error = %e, "failed to alert user");
        }
    }
}

pub fn notify_user(title: &str, message: &str) {
    tracing::info!(title = %title, message = %message, "notifying user");
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = tauri_winrt_notification::Toast::new(crate::brand::brand().aumid)
            .title(title)
            .text1(message)
            .duration(tauri_winrt_notification::Duration::Short)
            .show()
        {
            tracing::warn!(error = %e, "toast notification failed");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let title = title.replace(['"', '\''], "");
        let message = message.replace(['"', '\''], "");
        let spawned = std::cfg_select! {
            target_os = "macos" => Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{message}\" with title \"{title}\""
                ))
                .status(),
            _ => Command::new("notify-send")
                .args([&title, &message])
                .status(),
        };
        if let Err(e) = spawned {
            tracing::error!(error = %e, "failed to notify user");
        }
    }
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
    let mut cmd = Command::new(program);
    cmd.args(prefix).arg(target);
    #[cfg(target_os = "windows")]
    crate::winproc::no_window(&mut cmd);
    match cmd.spawn() {
        Ok(_) => {},
        Err(e) => tracing::error!(target = %target, program, error = %e, "failed to spawn opener"),
    }
}
