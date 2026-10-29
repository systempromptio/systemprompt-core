//! The two `OpenCode` packagings as
//! [`HostApp`](crate::integration::host_app::HostApp)s over the shared config
//! writer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::host::{HostAppKind, HostKind};

use super::tier::{ProviderTier, provider_tier};
use super::{config, install, probe};
use crate::drift::DesiredProfile;
use crate::integration::app_launch;
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppSnapshot,
    HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe, ProfileRemoval,
    ProfileState,
};

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeAppHost;

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeCliHost;

pub static OPENCODE_APP_HOST: OpenCodeAppHost = OpenCodeAppHost;
pub static OPENCODE_CLI_HOST: OpenCodeCliHost = OpenCodeCliHost;

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
    let found = HostProcesses::from_enumeration(probe::list_opencode_processes(), packaging);
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
    let token = Some(inputs.host_token.as_str());
    let read = if provider_tier() == ProviderTier::User {
        let rendered = serde_json::to_value(install::render(inputs))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        probe::keys_of(
            &probe::with_token(rendered, token),
            "rendered profile",
            config::KEYS_OF_INTEREST,
        )
    } else {
        probe::keys_of(
            &probe::with_token(serde_json::Value::Object(serde_json::Map::new()), token),
            "rendered profile",
            config::MANAGED_KEYS_OF_INTEREST,
        )
    };
    Ok(read.desired(config::role_of))
}

const fn install_action_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "merged into %USERPROFILE%\\.config\\opencode\\opencode.json"
    } else {
        "merged into ~/.config/opencode/opencode.json"
    }
}

const fn description() -> &'static str {
    "The open-source OpenCode coding agent. systemprompt-bridge writes the configuration that \
     routes inference through the gateway, registers MCP connectors, and publishes managed \
     skills, agents, plugins and rules."
}

impl HostApp for OpenCodeAppHost {
    fn id(&self) -> HostKind {
        HostKind::OpenCode
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
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
        Ok(install::write_profile(inputs)?)
    }

    fn install_profile(
        &self,
        path: &str,
        _gate: &crate::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn install_profile_unattended(
        &self,
        path: &str,
        _gate: &crate::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        install::remove_profile()
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

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        "https://opencode.ai/"
    }
}

impl HostApp for OpenCodeCliHost {
    fn id(&self) -> HostKind {
        HostKind::OpenCodeCli
    }

    fn display_name(&self) -> &'static str {
        "OpenCode CLI"
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
        Ok(install::write_profile(inputs)?)
    }

    fn install_profile(
        &self,
        path: &str,
        _gate: &crate::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn install_profile_unattended(
        &self,
        path: &str,
        _gate: &crate::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        install::remove_profile()
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

    // Why: one product logo serves both packagings.
    fn icon_id(&self) -> &'static str {
        HostKind::OpenCode.as_str()
    }

    fn profile_identity(&self) -> HostKind {
        super::TOKEN_HOST
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        "https://opencode.ai/docs/cli"
    }
}
