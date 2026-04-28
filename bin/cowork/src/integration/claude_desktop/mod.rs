mod gateway_probe;
mod managed_prefs;
mod process;
mod profile;

use serde::Serialize;

pub use gateway_probe::{GatewayHealth, GatewayProbeState};
pub use managed_prefs::{ManagedDomain, ManagedPrefsState};
pub use profile::{
    GenerateProfileBody, GeneratedProfile, ProfileGenInputs, default_models, install_profile,
    write_profile,
};

const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";
const CODE_DOMAIN: &str = "com.anthropic.claudecode";

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
        desktop: managed_prefs::read_domain(
            DESKTOP_DOMAIN,
            &[
                "inferenceProvider",
                "inferenceGatewayBaseUrl",
                "inferenceGatewayApiKey",
                "inferenceModels",
            ],
        ),
        code: managed_prefs::read_domain(CODE_DOMAIN, &[]),
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

    let claude_processes = process::list_claude_processes();
    ClaudeIntegrationSnapshot {
        managed_prefs,
        gateway_health,
        claude_running: !claude_processes.is_empty(),
        claude_processes,
        probed_at_unix: process::now_unix(),
    }
}
