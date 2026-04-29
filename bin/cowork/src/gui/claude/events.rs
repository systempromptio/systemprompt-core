use crate::gui::error::GuiError;
use crate::integration::claude_desktop::{ClaudeIntegrationSnapshot, GeneratedProfile};

#[derive(Debug, Clone)]
pub enum ClaudeUiEvent {
    ProbeRequested,
    ProbeFinished(Box<ClaudeIntegrationSnapshot>),
    ProfileGenerateRequested,
    ProfileGenerateFinished(Result<GeneratedProfile, GuiError>),
    ProfileInstallRequested(String),
    ProfileInstallFinished(Result<String, GuiError>),
}
