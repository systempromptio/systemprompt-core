use crate::integration::claude_desktop::ClaudeIntegrationSnapshot;

#[derive(Debug, Clone, Default)]
pub struct ClaudeState {
    pub integration: Option<ClaudeIntegrationSnapshot>,
    pub probe_in_flight: bool,
    pub last_generated_profile: Option<String>,
}
