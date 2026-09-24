//! `OpenCode` host integration: config, install, managed resources, probing.
//!
//! `OpenCode` merges config layers with the admin-controlled managed directory
//! (`/etc/opencode`, `/Library/Application Support/opencode`,
//! `%ProgramData%\opencode`) above every user and project file, so the
//! bridge-owned `provider.systemprompt` block and default `model` are written
//! there rather than somewhere a personal config casually overrides. That is
//! tier preference, not enforcement: the file is mode 0644, Linux falls back to
//! the user tier when `/etc` is unwritable, every platform writes the user tier
//! when an existing admin file is read-only to this process (so its stale
//! catalogue is at least extended), and nothing stops a user adding
//! another provider. Governance is enforced at the gateway. The API
//! key lives in the user's `auth.json`, and MCP connectors and skills — which
//! unattended sync must be able to rewrite without a prompt — stay in the
//! user's global config and skills directory.
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

use crate::integration::host_app::{
    ConfigFormat, Freshness, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema,
    HostKind, HostProcesses, ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileProbe,
    ProfileRemoval, ProfileState,
};
use crate::integration::reapply::Attendance;

#[must_use]
pub fn admin_tier_models() -> Option<(std::path::PathBuf, Vec<String>)> {
    install::admin_tier_models()
}

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeHost;

pub static OPENCODE_HOST: OpenCodeHost = OpenCodeHost;

impl HostApp for OpenCodeHost {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    fn config_schema(&self) -> &'static HostConfigSchema {
        &config::SCHEMA
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let read = probe::read_config();
        let endpoint = ProfileState::endpoint_freshness(
            read.keys.get(config::PROVIDER_BASE_URL).map(String::as_str),
            env.proxy_port,
        );
        // Why: auth.json holds the opencode host token; a stale one means the
        // proxy will attribute OpenCode traffic as an unverified secret holder
        // until sync re-renders it, so it must surface as Stale.
        let secret = Freshness::compare(
            install::installed_key_fingerprint(&config::auth_json_path()).as_deref(),
            env.host_token_fingerprint(&crate::ids::HostId::new(self.id()))
                .as_deref(),
            "opencode host token",
        );
        let profile_state = ProfileState::classify(&ProfileProbe {
            required: config::REQUIRED_KEYS,
            present: &read.keys,
            read_error: read.probe_error.as_deref(),
            secret,
            endpoint,
            managed_servers: Freshness::Unchecked,
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
            app_installed: crate::integration::app_launch::cli_installed(
                config::BINARY,
                &config::extra_bin_dirs(),
            ),
            probed_at_unix: config::now_unix(),
            update_needs_approval: false,
        }
    }

    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        install::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> std::io::Result<ProfileInstalled> {
        install::install_profile(path, Attendance::Attended)
    }

    fn install_profile_unattended(&self, path: &str) -> std::io::Result<ProfileInstalled> {
        install::install_profile(path, Attendance::Unattended)
    }

    fn remove_profile(&self) -> std::io::Result<ProfileRemoval> {
        install::remove_profile()
    }

    fn can_open(&self) -> bool {
        false
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "merged into /Library/Application Support/opencode/opencode.json (API key in \
             ~/.local/share/opencode/auth.json)"
        } else if cfg!(target_os = "windows") {
            "merged into %ProgramData%\\opencode\\opencode.json (API key in \
             %USERPROFILE%\\.local\\share\\opencode\\auth.json)"
        } else {
            "merged into /etc/opencode/opencode.json (API key in ~/.local/share/opencode/auth.json)"
        }
    }

    fn kind(&self) -> HostKind {
        HostKind::CliTool
    }

    fn description(&self) -> &'static str {
        "The open-source OpenCode coding agent (terminal, desktop and IDE). systemprompt-bridge \
         installs admin-managed configuration that routes inference through the gateway, \
         registers MCP connectors, and publishes managed skills."
    }

    fn icon_id(&self) -> &'static str {
        "opencode"
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
