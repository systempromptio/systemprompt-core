//! Codex host integration (CLI, desktop App, IDE Extension).
//!
//! Codex ships a managed-configuration layer that takes precedence over user
//! config and applies uniformly to the CLI, the desktop app, and the IDE
//! extension. systemprompt-bridge writes the managed configuration so a single
//! install governs all three surfaces.
//!
//! Managed config locations:
//! - macOS: MDM `.mobileconfig` payload with `PayloadType = com.openai.codex`
//!   and key `config_toml_base64` (base64-encoded TOML).
//! - Linux: `/etc/codex/managed_config.toml` (admin-owned; install requires sudo
//!   on machines without write access to `/etc/codex`).
//! - Windows: `~/.codex/managed_config.toml` (read by the app at managed
//!   precedence; user-writable but app honors precedence over `config.toml`).
//!
//! Reference: https://developers.openai.com/codex/enterprise/managed-configuration
//!
//! The managed file is owned entirely by systemprompt-bridge; we do not attempt
//! to merge with user config. Removing the managed config falls back to the
//! user's `config.toml` automatically.

mod config;
mod install;
mod probe;

use crate::integration::host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, HostKind,
    ProfileGenInputs, ProfileState,
};

pub struct CodexCliHost;

pub static CODEX_CLI_HOST: CodexCliHost = CodexCliHost;

impl HostApp for CodexCliHost {
    fn id(&self) -> &'static str {
        "codex-cli"
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &config::SCHEMA
    }

    fn probe(&self) -> HostAppSnapshot {
        let read = probe::read_config();
        let profile_state = ProfileState::from_keys(config::REQUIRED_KEYS, &read.keys);
        let processes = probe::list_codex_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            probed_at_unix: config::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        install::install_profile(path)
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "loaded into managed preferences (com.openai.codex)"
        } else if cfg!(target_os = "windows") {
            "written to %USERPROFILE%\\.codex\\managed_config.toml"
        } else {
            "written to /etc/codex/managed_config.toml"
        }
    }

    fn kind(&self) -> HostKind {
        HostKind::CliTool
    }

    fn description(&self) -> &'static str {
        "OpenAI's Codex (CLI, desktop app, IDE extension). systemprompt-bridge installs managed configuration that takes precedence over user config across all three surfaces."
    }

    fn icon_id(&self) -> &'static str {
        "codex-cli"
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }
}
