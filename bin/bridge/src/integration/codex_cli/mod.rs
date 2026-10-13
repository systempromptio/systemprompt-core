//! Codex CLI host integration: config, install, managed resources, probing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod install;
mod managed_resources;
mod probe;

pub use managed_resources::CodexCliSync;

use systemprompt_models::bridge::host::HostKind;

use crate::drift::DesiredProfile;
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppKind,
    HostAppSnapshot, HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe,
    ProfileRemoval, ProfileState,
};

#[derive(Clone, Copy, Debug)]
pub struct CodexCliHost;

pub static CODEX_CLI_HOST: CodexCliHost = CodexCliHost;

impl HostApp for CodexCliHost {
    fn id(&self) -> HostKind {
        HostKind::CodexCli
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    // Why: only the managed config is governance; a provider block the user
    // wrote in their own config.toml is theirs to change and is not read.
    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let read = probe::read_config();
        let desired = env.desired_for(self);
        let profile_state = ProfileState::classify(&ProfileProbe {
            present: &read.keys,
            observed: &read.observed,
            read_error: read.probe_error.as_deref(),
            desired: &desired,
        });
        let found = HostProcesses::from_enumeration(probe::list_codex_processes());
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            probe_error: read.probe_error.or(found.error),
            host_running: found.running,
            host_processes: found.processes,
            app_installed: self.app_installed(&env.start_menu),
            probed_at_unix: config::now_unix(),
            update_needs_approval: false,
        }
    }

    fn desired(&self, inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        let rendered: toml::Value = toml::from_str(&install::managed_toml(inputs)?)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(probe::keys_of(&rendered, "rendered profile").desired(config::role_of))
    }

    fn app_installed(&self, start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState {
        crate::integration::app_launch::is_installed(&locator(), start_menu)
    }

    fn generate_profile(
        &self,
        inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path).map(|()| ProfileInstalled::ok())
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        Ok(install::remove_profile()?)
    }

    fn open(&self) -> Result<(), HostAppError> {
        Ok(crate::integration::app_launch::open_app(&locator())?)
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "offered to System Settings — approve it under General › Device Management"
        } else if cfg!(target_os = "windows") {
            "merged into %USERPROFILE%\\.codex\\managed_config.toml"
        } else {
            "merged into /etc/codex/config.toml"
        }
    }

    fn kind(&self) -> HostAppKind {
        HostAppKind::CliTool
    }

    fn description(&self) -> &'static str {
        "OpenAI's Codex (CLI, desktop app, IDE extension). systemprompt-bridge installs managed \
         configuration that takes precedence over user config across all three surfaces."
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }

    fn download_url(&self) -> &'static str {
        "https://developers.openai.com/codex/app"
    }
}

const fn locator() -> crate::integration::app_launch::AppLocator<'static> {
    crate::integration::app_launch::AppLocator {
        macos_name: "Codex",
        windows_name: "Codex",
        windows_candidates: &[],
        linux_bin: "codex",
        msix_family: None,
        msix_app_id: "App",
    }
}

crate::register_host_sync!(CodexCliSync);

pub(crate) use managed_resources::feedback_skill_roots;
