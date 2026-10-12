//! `OpenCode` host integration: config, install, managed resources, probing.
//!
//! `OpenCode` 2.x loads only the user's global config
//! (`~/.config/opencode/opencode.json`, on every platform), so the
//! bridge-owned `provider.systemprompt` block — `apiKey` included — the
//! `enabled_providers` allowlist and the default `model` are merged there,
//! beside the MCP connectors. That is placement, not enforcement: nothing stops
//! a user editing the file or connecting another provider, and sync rewrites
//! it. Governance is enforced at the gateway.
//!
//! `OpenCode` takes the default `accepted_surfaces` — the empty slice, meaning
//! every advertised provider. `OpenCode` speaks the OpenAI-compatible wire and
//! the gateway serves it at `/v1/chat/completions`, but the gateway normalises
//! any inbound wire to canonical and renders any provider wire outbound, so a
//! gemini- or anthropic-native provider is just as servable over that one wire.
//! Filtering by the provider's native family hid every working `claude-*` and
//! `gemini-*` model from the picker while advertising only providers whose
//! credentials were unresolvable. `Backend` is still absent: it is excluded
//! from the advertised set itself, precisely to hide a provider from every
//! picker.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod install;
mod managed_resources;
mod probe;

pub use managed_resources::OpenCodeSync;

use systemprompt_models::bridge::host::HostKind;

use crate::drift::DesiredProfile;
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppKind,
    HostAppSnapshot, HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe,
    ProfileRemoval, ProfileState,
};

// Why: the CLI and the desktop app are two installs of one host; both read
// the same user config, so either one counts, and only a conclusive miss on
// both is a miss.
#[must_use]
pub const fn install_state(cli: AppInstallState, desktop: AppInstallState) -> AppInstallState {
    match (cli, desktop) {
        (AppInstallState::Installed, _) | (_, AppInstallState::Installed) => {
            AppInstallState::Installed
        },
        (AppInstallState::NotInstalled, AppInstallState::NotInstalled) => {
            AppInstallState::NotInstalled
        },
        _ => AppInstallState::Unknown,
    }
}

fn desktop_app_present() -> bool {
    let candidates = config::desktop_candidates();
    crate::integration::app_launch::desktop_present_on_disk(&config::desktop_locator(&candidates))
}

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeHost;

pub static OPENCODE_HOST: OpenCodeHost = OpenCodeHost;

impl HostApp for OpenCodeHost {
    fn id(&self) -> HostKind {
        HostKind::OpenCode
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let read = probe::read_config();
        let desired = env.desired_for(self);
        let profile_state = ProfileState::classify(&ProfileProbe {
            present: &read.keys,
            observed: &read.observed,
            read_error: read.probe_error.as_deref(),
            desired: &desired,
        });
        let found = HostProcesses::from_enumeration(probe::list_opencode_processes());
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
        let rendered: serde_json::Value =
            serde_json::from_str(&install::provider_json_text(inputs)?)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(probe::keys_of(&rendered, "rendered profile").desired(config::role_of))
    }

    fn app_installed(&self, start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState {
        let desktop_candidates = config::desktop_candidates();
        install_state(
            crate::integration::app_launch::cli_installed(
                config::BINARY,
                &config::extra_bin_dirs(),
            ),
            crate::integration::app_launch::is_installed(
                &config::desktop_locator(&desktop_candidates),
                start_menu,
            ),
        )
    }

    fn generate_profile(
        &self,
        inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        Ok(install::write_profile(inputs)?)
    }

    fn install_profile(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn install_profile_unattended(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        install::install_profile(path)
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        install::remove_profile()
    }

    fn open(&self) -> Result<(), HostAppError> {
        let candidates = config::desktop_candidates();
        Ok(crate::integration::app_launch::open_app(
            &config::desktop_locator(&candidates),
        )?)
    }

    fn can_open(&self) -> bool {
        desktop_app_present()
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "merged into %USERPROFILE%\\.config\\opencode\\opencode.json"
        } else {
            "merged into ~/.config/opencode/opencode.json"
        }
    }

    fn kind(&self) -> HostAppKind {
        HostAppKind::CliTool
    }

    fn description(&self) -> &'static str {
        "The open-source OpenCode coding agent (terminal, desktop and IDE). systemprompt-bridge \
         writes the configuration that routes inference through the gateway, \
         registers MCP connectors, and publishes managed skills."
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        "https://opencode.ai/"
    }
}

crate::register_host_sync!(OpenCodeSync);

pub(crate) fn feedback_skill_root() -> std::path::PathBuf {
    config::skills_dir()
}

#[must_use]
pub fn retired_admin_tier() -> Option<std::path::PathBuf> {
    install::stale_admin_tier()
}
