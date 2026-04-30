//! Codex CLI host integration.
//!
//! Codex (github.com/openai/codex) is a CLI coding agent. Cowork manages a defined
//! set of dotted TOML keys inside `~/.codex/config.toml` to route inference through
//! the systemprompt gateway, redirect OpenTelemetry, disable upstream analytics, and
//! configure cowork itself as the credential helper via Codex's `auth.command`
//! contract.
//!
//! Limitation: stock `toml = "0.8"` round-tripping drops user comments and reorders
//! keys. Acceptable trade-off for v1; migrate to `toml_edit` if it bites a user.
//!
//! Future enterprise lock (out of scope v1): add a `LockableHostApp` trait method
//! that writes to a system-owned config path. Two viable mechanisms — a
//! Codex-supported system precedence path (does not exist upstream today), or a
//! shim binary that forces `CODEX_HOME` to a root-owned directory.

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
        if cfg!(target_os = "windows") {
            "merged into %USERPROFILE%\\.codex\\config.toml"
        } else {
            "merged into ~/.codex/config.toml"
        }
    }

    fn kind(&self) -> HostKind {
        HostKind::CliTool
    }

    fn description(&self) -> &'static str {
        "OpenAI's open-source coding CLI. Cowork merges managed gateway, OTEL, and credential-helper keys into ~/.codex/config.toml."
    }

    fn icon_id(&self) -> &'static str {
        "codex-cli"
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }
}
