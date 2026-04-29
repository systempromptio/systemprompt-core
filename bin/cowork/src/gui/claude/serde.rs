use serde::Serialize;

use crate::gui::state::AppStateSnapshot;
use crate::integration::claude_desktop::ClaudeIntegrationSnapshot;

#[derive(Serialize)]
pub(crate) struct ClaudePayload<'a> {
    pub claude_integration: Option<&'a ClaudeIntegrationSnapshot>,
    pub last_generated_profile: Option<&'a str>,
}

pub(crate) fn payload(snap: &AppStateSnapshot) -> ClaudePayload<'_> {
    ClaudePayload {
        claude_integration: snap.claude.integration.as_ref(),
        last_generated_profile: snap.claude.last_generated_profile.as_deref(),
    }
}
