mod shared;
pub mod win_reg_parser;

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
        let profile_state = ProfileState::from_keys(shared::REQUIRED_KEYS, &read.keys);
        let processes = os::list_claude_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            probed_at_unix: shared::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        os::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        os::install_profile(path)
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
        "Anthropic's official desktop client for Claude. Routes inference through the systemprompt gateway via managed policy."
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
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn proxy_url_from_keys(snap: &HostAppSnapshot) -> Option<&str> {
    snap.profile_keys
        .get("inferenceGatewayBaseUrl")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
}
