//! The two Codex packagings as
//! [`HostApp`](crate::integration::host_app::HostApp)s over the shared config
//! writer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::{HostAppKind, HostKind};

use super::{config, install, probe};
use crate::drift::DesiredProfile;
use crate::integration::app_launch;
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppSnapshot,
    HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe, ProfileRemoval,
    ProfileState,
};

#[derive(Clone, Copy, Debug)]
pub struct CodexAppHost;

#[derive(Clone, Copy, Debug)]
pub struct CodexCliHost;

pub static CODEX_APP_HOST: CodexAppHost = CodexAppHost;
pub static CODEX_CLI_HOST: CodexCliHost = CodexCliHost;

// Why: only the managed config is governance; a provider block the user
// wrote in their own config.toml is theirs to change and is not read.
fn snapshot(
    host: &dyn HostApp,
    env: &ProbeEnv,
    packaging: HostAppKind,
    app_installed: AppInstallState,
) -> HostAppSnapshot {
    let read = probe::read_config();
    let desired = env.desired_for(host);
    let profile_state = ProfileState::classify(&ProfileProbe {
        present: &read.keys,
        observed: &read.observed,
        read_error: read.probe_error.as_deref(),
        desired: &desired,
    });
    let found = HostProcesses::from_enumeration(probe::list_codex_processes(), packaging);
    HostAppSnapshot {
        host_id: host.id(),
        display_name: host.display_name(),
        profile_state,
        profile_source: read.source_path,
        profile_keys: read.keys,
        probe_error: read.probe_error.or(found.error),
        host_running: found.running,
        host_processes: found.processes,
        app_installed,
        probed_at_unix: config::now_unix(),
        update_needs_approval: false,
    }
}

fn desired(inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
    let rendered: toml::Value = toml::from_str(&install::managed_toml(inputs)?)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(probe::keys_of(&rendered, "rendered profile").desired(config::role_of))
}

const fn install_action_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "offered to System Settings — approve it under General › Device Management"
    } else if cfg!(target_os = "windows") {
        "merged into %USERPROFILE%\\.codex\\managed_config.toml"
    } else {
        "merged into /etc/codex/config.toml"
    }
}

const fn description() -> &'static str {
    "OpenAI's Codex. systemprompt-bridge installs managed configuration that takes precedence \
     over user config for the desktop app, the CLI and the IDE extension alike."
}

impl HostApp for CodexAppHost {
    fn id(&self) -> HostKind {
        HostKind::Codex
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        snapshot(
            self,
            env,
            HostAppKind::DesktopApp,
            self.app_installed(&env.start_menu),
        )
    }

    fn desired(&self, inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        desired(inputs)
    }

    fn app_installed(&self, start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState {
        let candidates = config::desktop_candidates();
        app_launch::desktop_installed(&config::desktop_locator(&candidates), start_menu)
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
        let candidates = config::desktop_candidates();
        Ok(app_launch::open_app(&config::desktop_locator(&candidates))?)
    }

    fn can_open(&self) -> bool {
        let candidates = config::desktop_candidates();
        app_launch::desktop_present_on_disk(&config::desktop_locator(&candidates))
    }

    fn install_action_label(&self) -> &'static str {
        install_action_label()
    }

    fn description(&self) -> &'static str {
        description()
    }

    // Why: one product logo serves both packagings.
    fn icon_id(&self) -> &'static str {
        HostKind::CodexCli.as_str()
    }

    fn profile_identity(&self) -> HostKind {
        super::TOKEN_HOST
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }

    fn download_url(&self) -> &'static str {
        "https://developers.openai.com/codex/app"
    }
}

impl HostApp for CodexCliHost {
    fn id(&self) -> HostKind {
        HostKind::CodexCli
    }

    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        snapshot(
            self,
            env,
            HostAppKind::CliTool,
            self.app_installed(&env.start_menu),
        )
    }

    fn desired(&self, inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        desired(inputs)
    }

    fn app_installed(&self, _start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState {
        app_launch::cli_installed(config::BINARY, &config::extra_bin_dirs())
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

    fn can_open(&self) -> bool {
        false
    }

    fn install_action_label(&self) -> &'static str {
        install_action_label()
    }

    fn description(&self) -> &'static str {
        description()
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Toml
    }

    fn download_url(&self) -> &'static str {
        "https://developers.openai.com/codex/cli"
    }
}
