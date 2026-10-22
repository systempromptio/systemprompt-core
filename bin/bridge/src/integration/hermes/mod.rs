//! Hermes Agent Desktop host integration: config, install, managed resources,
//! probing.
//!
//! Nous Research's "Hermes Agent Desktop" reads a plain `config.yaml` plus an
//! `.env` from `HERMES_HOME` on every OS, so — unlike the Codex host — there is
//! no macOS `.mobileconfig` path: the profile is written directly everywhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod install;
mod managed_resources;
mod probe;

pub use managed_resources::HermesSync;

/// The bridge-owned keys of the Hermes profile, named so a consumer can
/// assert the contract without copying string literals.
pub mod contract {
    pub use super::config::{
        API_MODE_VALUE, ENV_API_KEY, MODEL_NAME, MODEL_PROVIDER, PROVIDER_API_MODE,
        PROVIDER_BASE_URL, PROVIDER_ENTRY, PROVIDER_KEY_ENV,
    };
}

pub use install::{install_profile_into, remove_profile_from};

use systemprompt_models::bridge::host::HostKind;

use crate::drift::{DesiredProfile, EntryRole};
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppSnapshot,
    HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe, ProfileRemoval,
    ProfileState,
};

#[derive(Clone, Copy, Debug)]
pub struct HermesHost;

pub static HERMES_HOST: HermesHost = HermesHost;

impl HostApp for HermesHost {
    fn id(&self) -> HostKind {
        HostKind::Hermes
    }

    fn display_name(&self) -> &'static str {
        "Hermes"
    }

    // Why: HERMES_HOME/.env holds the hermes host token; a stale one leaves
    // Hermes traffic attributed as an unverified secret holder until sync
    // re-renders it, so it is part of what "current" means.
    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let mut read = probe::read_config();
        if let Some(fp) =
            install::installed_key_fingerprint(&config::env_path_in(&config::hermes_home()))
        {
            read.observed.insert(config::ENV_KEY_ENTRY.to_owned(), fp);
        }
        let desired = env.desired_for(self);
        let profile_state = ProfileState::classify(&ProfileProbe {
            present: &read.keys,
            observed: &read.observed,
            read_error: read.probe_error.as_deref(),
            desired: &desired,
        });
        let found = HostProcesses::from_enumeration(
            probe::list_hermes_processes(),
            systemprompt_models::bridge::host::HostAppKind::CliTool,
        );
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
        let rendered: serde_yaml::Value = serde_yaml::from_str(&install::managed_yaml(inputs)?)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut desired = probe::keys_of(&rendered, "rendered profile").desired(config::role_of);
        desired.push(
            config::ENV_KEY_ENTRY,
            crate::proxy::secret::fingerprint(inputs.host_token.as_str()),
            EntryRole::Secret,
        );
        Ok(desired)
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
        "merged into HERMES_HOME/config.yaml as a named `providers:` entry (with OPENAI_API_KEY \
         written to HERMES_HOME/.env)"
    }

    fn description(&self) -> &'static str {
        "Nous Research's Hermes Agent Desktop. systemprompt-bridge writes managed configuration \
         that routes inference through the gateway, registers MCP connectors, and publishes \
         managed skills."
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Yaml
    }

    fn download_url(&self) -> &'static str {
        "https://nousresearch.com/"
    }
}

const fn locator() -> crate::integration::app_launch::AppLocator<'static> {
    crate::integration::app_launch::AppLocator {
        macos_name: "Hermes",
        windows_name: "Hermes",
        windows_candidates: &[],
        linux_bin: config::BINARY,
        msix_family: None,
        msix_app_id: "App",
    }
}

crate::register_host_sync!(HermesSync);

pub(crate) fn feedback_skill_root() -> std::path::PathBuf {
    config::skills_dir()
}
