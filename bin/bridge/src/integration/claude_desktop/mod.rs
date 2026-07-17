//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod reg_profile;
mod shared;

#[cfg(target_os = "windows")]
pub(crate) mod elevate;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

pub use shared::{ProfileGenInputs, default_models};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, HostKind,
    ProfileState,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
pub struct ClaudeDesktopHost;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub static CLAUDE_DESKTOP_HOST: ClaudeDesktopHost = ClaudeDesktopHost;

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl HostApp for ClaudeDesktopHost {
    fn id(&self) -> &'static str {
        "claude-desktop"
    }

    fn display_name(&self) -> &'static str {
        "Claude Desktop"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &shared::SCHEMA
    }

    fn probe(&self) -> HostAppSnapshot {
        let read = os::read_domain(shared::DESKTOP_DOMAIN);
        let secret_fresh = shared::secret_freshness(read.api_key_fp.as_deref());
        let profile_state = ProfileState::classify(shared::REQUIRED_KEYS, &read.keys, secret_fresh);
        let processes = os::list_claude_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            app_installed: crate::integration::app_launch::is_installed(
                "Claude",
                "Claude",
                &claude_app_candidates(),
                "claude",
            ),
            probed_at_unix: shared::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        os::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        os::install_profile(path)
    }

    fn open(&self) -> std::io::Result<()> {
        crate::integration::app_launch::open_app(
            "Claude",
            "Claude",
            &claude_app_candidates(),
            "claude",
        )
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "imported into Windows Registry"
        } else {
            "loaded into managed preferences"
        }
    }

    fn kind(&self) -> HostKind {
        HostKind::DesktopApp
    }

    fn description(&self) -> &'static str {
        "Anthropic's official desktop client for Claude. Routes inference through the systemprompt \
         gateway via managed policy."
    }

    fn icon_id(&self) -> &'static str {
        "claude-desktop"
    }

    fn config_format(&self) -> ConfigFormat {
        if cfg!(target_os = "windows") {
            ConfigFormat::Reg
        } else {
            ConfigFormat::Plist
        }
    }

    fn download_url(&self) -> &'static str {
        "https://claude.ai/download"
    }

    fn accepted_surfaces(&self) -> &'static [systemprompt_models::profile::ApiSurface] {
        &[systemprompt_models::profile::ApiSurface::Anthropic]
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn claude_app_candidates() -> Vec<std::path::PathBuf> {
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn proxy_url_from_keys(snap: &HostAppSnapshot) -> Option<&str> {
    snap.profile_keys
        .get("inferenceGatewayBaseUrl")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
}
