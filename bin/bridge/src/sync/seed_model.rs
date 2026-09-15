//! Seeds the Claude Code model picker and default model from the bridge
//! profile once a sync has completed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SyncError;

pub(super) async fn seed_default_model_from_profile(
    client: &crate::gateway::GatewayClient,
) -> Result<(), SyncError> {
    let profile = match client.fetch_bridge_profile().await {
        Ok(profile) => profile,
        // Why: a gateway older than the profile endpoint has no default model
        // to seed; the sync itself completed and its checkpoint is written.
        Err(crate::gateway::GatewayError::HttpStatus {
            status: reqwest::StatusCode::NOT_FOUND,
            ..
        }) => return Ok(()),
        Err(e) => return Err(SyncError::Gateway(e)),
    };
    let rows =
        crate::install::mdm::claude_code_settings::model_picker::picker_rows(&profile.providers);
    match crate::install::mdm::claude_code_settings::apply_model_picker(&rows) {
        Ok(lines) => {
            for line in lines {
                tracing::info!(target: "bridge::install", detail = %line, "claude code model picker");
            }
        },
        Err(source) => {
            return Err(SyncError::ClaudeCodeSettings {
                what: "model picker",
                source,
            });
        },
    }
    let Some(model) = profile.default_model.as_deref() else {
        return Ok(());
    };
    match crate::install::mdm::claude_code_settings::seed_default_model(model) {
        Ok(true) => tracing::info!(model, "seeded the default model from the bridge profile"),
        Ok(false) => tracing::debug!("settings already name a model; leaving the user's choice"),
        Err(source) => {
            return Err(SyncError::ClaudeCodeSettings {
                what: "default model seed",
                source,
            });
        },
    }
    Ok(())
}
