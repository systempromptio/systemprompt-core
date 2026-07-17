//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod install;
mod managed_resources;
mod probe;

pub use managed_resources::CodexCliSync;

use crate::integration::host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, HostKind,
    ProfileGenInputs, ProfileState,
};

#[derive(Clone, Copy, Debug)]
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
        let profile_state = ProfileState::classify(config::REQUIRED_KEYS, &read.keys, None);
        let processes = probe::list_codex_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            app_installed: crate::integration::app_launch::is_installed(
                "Codex",
                "Codex",
                &[],
                "codex",
            ),
            probed_at_unix: config::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        install::install_profile(path)
    }

    fn open(&self) -> std::io::Result<()> {
        crate::integration::app_launch::open_app("Codex", "Codex", &[], "codex")
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "loaded into managed preferences (com.openai.codex)"
        } else if cfg!(target_os = "windows") {
            "merged into %USERPROFILE%\\.codex\\managed_config.toml"
        } else {
            "merged into /etc/codex/config.toml"
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

    fn download_url(&self) -> &'static str {
        "https://developers.openai.com/codex/app"
    }

    fn accepted_surfaces(&self) -> &'static [systemprompt_models::profile::ApiSurface] {
        &[]
    }
}
