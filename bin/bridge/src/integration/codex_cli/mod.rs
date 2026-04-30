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
        "OpenAI's Codex (CLI, desktop app, IDE extension). systemprompt-bridge installs managed \
         configuration that takes precedence over user config across all three surfaces."
    }

    fn icon_id(&self) -> &'static str {
        "codex-cli"
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }
}
