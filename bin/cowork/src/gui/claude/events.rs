use crate::integration::claude_desktop::{ClaudeIntegrationSnapshot, GeneratedProfile};

#[derive(Debug, Clone)]
pub enum ClaudeUiEvent {
    ProbeRequested,
    ProbeFinished(Box<ClaudeIntegrationSnapshot>),
    ProfileGenerateRequested,
    ProfileGenerateFinished(Result<GeneratedProfile, String>),
    ProfileInstallRequested(String),
    ProfileInstallFinished(Result<String, String>),
}
