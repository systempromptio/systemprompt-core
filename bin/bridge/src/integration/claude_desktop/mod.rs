//! Claude Desktop host integration, split per OS with an elevation helper.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod reg_profile;
mod shared;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_read;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

pub use shared::{PlistValueTexts, ProfileGenInputs, default_models, plist_value_texts};

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[must_use]
pub(crate) fn policy_summary() -> Vec<String> {
    let read = os::read_domain(shared::DESKTOP_DOMAIN, &shared::keys_to_read(None));
    let mut lines = vec![format!(
        "source: {}",
        read.source_path.as_deref().unwrap_or("<no managed policy>")
    )];
    if let Some(error) = read.probe_error.as_deref() {
        lines.push(format!("read error: {error}"));
    }
    if let Some(fp) = read.api_key_fp.as_deref() {
        lines.push(format!("api key fingerprint: {fp}"));
    }
    lines.extend(read.keys.iter().map(|(k, v)| format!("{k} = {v}")));
    lines
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[must_use]
pub(crate) fn policy_summary() -> Vec<String> {
    vec!["<no managed policy on this platform>".to_owned()]
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
use systemprompt_models::bridge::host::HostKind;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::drift::DesiredProfile;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::host_app::{
    AppInstallState, ConfigFormat, GeneratedProfile, HostApp, HostAppError, HostAppKind,
    HostAppSnapshot, HostProcesses, ProbeEnv, ProfileInstalled, ProfileRemoval, ProfileState,
};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::integration::profile_state::ProfileProbe;

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
pub struct ClaudeDesktopHost;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub static CLAUDE_DESKTOP_HOST: ClaudeDesktopHost = ClaudeDesktopHost;

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl HostApp for ClaudeDesktopHost {
    fn id(&self) -> HostKind {
        HostKind::ClaudeDesktop
    }

    fn display_name(&self) -> &'static str {
        "Claude Desktop"
    }

    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot {
        let desired = env.desired_for(self);
        let read = os::read_domain(
            shared::DESKTOP_DOMAIN,
            &shared::keys_to_read(desired.as_ref().ok()),
        );
        let profile_state = ProfileState::classify(&ProfileProbe {
            present: &read.keys,
            observed: &read.observed,
            read_error: read.probe_error.as_deref(),
            desired: &desired,
        });
        let found = HostProcesses::from_enumeration(os::list_claude_processes());
        let update_needs_approval = os::update_needs_approval(read.source_path.as_deref(), env);
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
            probed_at_unix: shared::now_unix(),
            update_needs_approval,
        }
    }

    fn desired(&self, inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        Ok(reg_profile::desired(inputs)?)
    }

    fn app_installed(&self, start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState {
        crate::integration::app_launch::is_installed(&locator(&claude_app_candidates()), start_menu)
    }

    fn generate_profile(
        &self,
        inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        os::write_profile(inputs)
    }

    fn install_profile(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        os::install_profile(path)
    }

    fn install_profile_unattended(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        os::install_profile_unattended(path)
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        os::remove_profile()
    }

    fn open(&self) -> Result<(), HostAppError> {
        Ok(crate::integration::app_launch::open_app(&locator(
            &claude_app_candidates(),
        ))?)
    }

    fn quit(&self) -> Result<(), HostAppError> {
        Ok(crate::integration::app_launch::quit_app(&locator(
            &claude_app_candidates(),
        ))?)
    }

    fn can_quit(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn install_action_label(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "imported into Windows Registry"
        } else {
            "offered to System Settings — approve it under General › Device Management"
        }
    }

    fn kind(&self) -> HostAppKind {
        HostAppKind::DesktopApp
    }

    fn description(&self) -> &'static str {
        "Anthropic's official desktop client for Claude. Routes inference through the systemprompt \
         gateway via managed policy."
    }

    fn profile_carries_managed_servers(&self) -> bool {
        true
    }

    // Why: the machine policy write is the one Desktop sync step that needs
    // an administrator, and its values are exactly what the probe compares.
    fn probe_observes(&self, emitter: &str, needs_elevation: bool) -> bool {
        needs_elevation && emitter == self.id().as_str()
    }

    fn config_format(&self) -> ConfigFormat {
        if cfg!(target_os = "windows") {
            ConfigFormat::Reg
        } else {
            ConfigFormat::Plist
        }
    }

    fn download_url(&self) -> &'static str {
        "https://claude.ai/download"
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
const MSIX_FAMILY: &str = crate::config::paths::CLAUDE_MSIX_FAMILY;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const MSIX_APP_ID: &str = "Claude";

#[cfg(any(target_os = "macos", target_os = "windows"))]
const fn locator(
    candidates: &[std::path::PathBuf],
) -> crate::integration::app_launch::AppLocator<'_> {
    crate::integration::app_launch::AppLocator {
        macos_name: "Claude",
        windows_name: "Claude",
        windows_candidates: candidates,
        linux_bin: "claude",
        msix_family: Some(MSIX_FAMILY),
        msix_app_id: MSIX_APP_ID,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn claude_app_candidates() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = std::path::PathBuf::from(local);
        out.push(
            local
                .join("Microsoft")
                .join("WindowsApps")
                .join("Claude.exe"),
        );
        out.push(local.join("AnthropicClaude").join("Claude.exe"));
    }
    out
}

#[must_use]
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn is_app_installed(start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    CLAUDE_DESKTOP_HOST.app_installed(start_menu).is_installed()
}

#[must_use]
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const fn is_app_installed(_start_menu: &crate::probe_cache::StartMenuCache) -> bool {
    false
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn proxy_url_from_keys(snap: &HostAppSnapshot) -> Option<&str> {
    snap.profile_keys
        .get("inferenceGatewayBaseUrl")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
}
