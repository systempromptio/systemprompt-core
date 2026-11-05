//! Locating the Claude Desktop app and the proxy URL its policy names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::HostApp as _;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn claude_app_candidates() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = std::path::PathBuf::from(local);
        out.push(
            local
                .join("Microsoft")
                .join("WindowsApps")
                .join("Claude.exe"),
        );
        out.push(local.join("AnthropicClaude").join("Claude.exe"));
    }
    out
}

#[must_use]
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn is_app_installed(start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    super::CLAUDE_DESKTOP_HOST
        .app_installed(start_menu)
        .is_installed()
}

#[must_use]
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const fn is_app_installed(_start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    false
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn proxy_url_from_keys(snap: &crate::integration::HostAppSnapshot) -> Option<&str> {
    snap.profile_keys
        .get("inferenceGatewayBaseUrl")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
}
