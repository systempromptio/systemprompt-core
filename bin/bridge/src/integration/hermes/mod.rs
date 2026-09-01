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

/// The bridge-owned keys, re-exported so a test can assert the contract with
/// Hermes by name rather than by copied string literal.
#[doc(hidden)]
pub mod contract {
    pub use super::config::{
        API_MODE_VALUE, ENV_API_KEY, MODEL_NAME, MODEL_PROVIDER, PROVIDER_API_MODE,
        PROVIDER_BASE_URL, PROVIDER_ENTRY, PROVIDER_KEY_ENV,
    };
}

#[doc(hidden)]
pub use install::{install_profile_into, remove_profile_from};

use crate::integration::host_app::{
    ConfigFormat, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, HostKind, ProbeEnv,
    ProfileGenInputs, ProfileRemoval, ProfileState,
};

#[derive(Clone, Copy, Debug)]
pub struct HermesHost;

pub static HERMES_HOST: HermesHost = HermesHost;

impl HostApp for HermesHost {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn display_name(&self) -> &'static str {
        "Hermes"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &config::SCHEMA
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let read = probe::read_config();
        // Why: like Codex, Hermes bakes `<origin>/v1`; the classifier ignores the
        // path and only checks that the loopback port still matches.
        let endpoint_fresh = ProfileState::endpoint_freshness(
            read.keys.get(config::PROVIDER_BASE_URL).map(String::as_str),
            env.proxy_port,
        );
        let profile_state =
            ProfileState::classify(config::REQUIRED_KEYS, &read.keys, None, endpoint_fresh);
        let processes = probe::list_hermes_processes();
        HostAppSnapshot {
            host_id: self.id(),
            display_name: self.display_name(),
            profile_state,
            profile_source: read.source_path,
            profile_keys: read.keys,
            host_running: !processes.is_empty(),
            host_processes: processes,
            app_installed: crate::integration::app_launch::is_installed(&locator()),
            probed_at_unix: config::now_unix(),
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<()> {
        install::install_profile(path)
    }

    fn remove_profile(&self) -> std::io::Result<ProfileRemoval> {
        install::remove_profile()
    }

    fn open(&self) -> std::io::Result<()> {
        crate::integration::app_launch::open_app(&locator())
    }

    fn install_action_label(&self) -> &'static str {
        "merged into HERMES_HOME/config.yaml as a named `providers:` entry (with OPENAI_API_KEY \
         written to HERMES_HOME/.env)"
    }

    fn kind(&self) -> HostKind {
        HostKind::DesktopApp
    }

    fn description(&self) -> &'static str {
        "Nous Research's Hermes Agent Desktop. systemprompt-bridge writes managed configuration \
         that routes inference through the gateway, registers MCP connectors, and publishes \
         managed skills."
    }

    fn icon_id(&self) -> &'static str {
        "hermes"
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Yaml
    }

    fn download_url(&self) -> &'static str {
        "https://nousresearch.com/"
    }

    // Why: the gateway serves Hermes over `/v1/chat/completions`; declaring the
    // OpenAI surface is what makes model negotiation offer compatible models and
    // the profile writer install the model half.
    fn accepted_surfaces(&self) -> &'static [systemprompt_models::profile::ApiSurface] {
        &[systemprompt_models::profile::ApiSurface::OpenAi]
    }
}

// Why: Hermes ships as a conventional installer on every platform, so there is
// no MSIX family to consult.
const fn locator() -> crate::integration::app_launch::AppLocator<'static> {
    crate::integration::app_launch::AppLocator {
        macos_name: "Hermes",
        windows_name: "Hermes",
        windows_candidates: &[],
        linux_bin: "hermes",
        msix_family: None,
        msix_app_id: "App",
    }
}

crate::register_host_sync!(HermesSync);
