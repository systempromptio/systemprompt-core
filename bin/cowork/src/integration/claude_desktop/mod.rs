mod gateway_probe;
mod shared;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

use serde::Serialize;

pub use gateway_probe::{GatewayHealth, GatewayProbeState};
pub use shared::{
    GenerateProfileBody, GeneratedProfile, ManagedDomain, ManagedPrefsState, ProfileGenInputs,
    default_models,
};

#[derive(Debug, Clone, Serialize, Default)]
pub struct ClaudeIntegrationSnapshot {
    pub managed_prefs: ManagedPrefsState,
    pub gateway_health: GatewayHealth,
    pub claude_running: bool,
    pub claude_processes: Vec<String>,
    pub probed_at_unix: u64,
}

pub fn probe() -> ClaudeIntegrationSnapshot {
    let managed_prefs = ManagedPrefsState {
        desktop: os::read_domain(
            shared::DESKTOP_DOMAIN,
            &[
                "inferenceProvider",
                "inferenceGatewayBaseUrl",
                "inferenceGatewayApiKey",
                "inferenceModels",
            ],
        ),
        code: os::read_domain(shared::CODE_DOMAIN, &[]),
    };

    let gateway_url = managed_prefs
        .desktop
        .keys
        .get("inferenceGatewayBaseUrl")
        .cloned();

    let gateway_health = match gateway_url.as_deref() {
        Some(url) if !url.is_empty() => gateway_probe::probe_gateway(url),
        _ => GatewayHealth {
            url: None,
            state: GatewayProbeState::Unconfigured,
            ..Default::default()
        },
    };

    let claude_processes = os::list_claude_processes();
    ClaudeIntegrationSnapshot {
        managed_prefs,
        gateway_health,
        claude_running: !claude_processes.is_empty(),
        claude_processes,
        probed_at_unix: shared::now_unix(),
    }
}

pub fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    os::write_profile(inputs)
}

pub fn install_profile(path: &str) -> std::io::Result<()> {
    os::install_profile(path)
}
