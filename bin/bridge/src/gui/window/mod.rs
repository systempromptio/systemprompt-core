//! Webview window management and external-target opening.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod native;
mod native_protocol;
mod native_webview;

pub use crate::window_state as geometry;

#[cfg(target_os = "windows")]
mod dwm;
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::process::Command;

pub use native::SettingsWindow;

use crate::wire::external_url::ExternalUrl;

#[cfg(target_os = "windows")]
pub use dwm::set_immersive_dark;

#[cfg(not(target_os = "windows"))]
pub const fn set_immersive_dark(_window: &dyn winit::window::Window, _dark: bool) {}

#[must_use]
pub fn prefers_dark(window: &dyn winit::window::Window) -> bool {
    !matches!(window.theme(), Some(winit::window::Theme::Light))
}

pub fn open_path(path: &Path) {
    if let Err(e) = opener::open(path) {
        tracing::error!(path = %path.display(), error = %e, "failed to open path");
    }
}

pub fn reveal_path(path: &Path) {
    if let Err(e) = opener::reveal(path) {
        tracing::error!(path = %path.display(), error = %e, "failed to reveal application");
    }
}

pub fn open_external_url(url: &ExternalUrl) {
    tracing::info!(url = %url, "opening external url");
    if let Err(e) = opener::open(url.as_str()) {
        tracing::error!(url = %url, error = %e, "failed to open external url");
    }
}

pub fn notify_user(title: &str, message: &str) {
    notify_user_with_action(title, message, None);
}

// Why: only a Windows toast can carry a click action; `osascript` and
// `notify-send` toasts open nothing, so on those platforms the link is
// reached from the inbox pane.
pub fn notify_user_with_action(title: &str, message: &str, open_url: Option<ExternalUrl>) {
    tracing::info!(title = %title, message = %message, has_link = open_url.is_some(), "notifying user");
    #[cfg(target_os = "windows")]
    {
        let mut toast = tauri_winrt_notification::Toast::new(crate::brand::brand().aumid)
            .title(title)
            .text1(message)
            .duration(tauri_winrt_notification::Duration::Short);
        if let Some(url) = open_url {
            toast = toast.on_activated(move |_action| {
                open_external_url(&url);
                Ok(())
            });
        }
        if let Err(e) = toast.show() {
            tracing::warn!(error = %e, "toast notification failed");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        drop(open_url);
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
