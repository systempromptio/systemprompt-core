//! `.reg` serialisation for the Claude Desktop managed-policy profile.
//!
//! The profile *is* the policy: the same
//! [`claude_desktop_policy`](crate::install::mdm::policy::claude_desktop_policy)
//! the machine sync enforces, so a repair that replaces the hive never leaves
//! a key the sync will then report as different. [`render_reg`] and
//! [`crate::install::reg_values::parse_reg_entries`] are inverses, kept
//! platform-independent so the round-trip is testable on every target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::shared::ProfileGenInputs;
use crate::install::mdm::MdmError;
use crate::install::mdm::policy::{PolicyInputs, claude_desktop_policy, reg_values};
use crate::install::reg_values::render_reg_values;

pub fn profile_entries(inputs: &ProfileGenInputs) -> Result<Vec<(&'static str, String)>, MdmError> {
    let models = if inputs.models.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&inputs.models).map_err(|e| MdmError::InvalidConfig {
                key: "inferenceModels",
                detail: e.to_string(),
            })?,
        )
    };
    let policy = claude_desktop_policy(&PolicyInputs {
        base_url: &inputs.gateway_base_url,
        host_token: &inputs.host_token,
        models,
        headers: &inputs.headers,
        egress_allowed_hosts: None,
        org_uuid: inputs.organization_uuid.as_deref(),
        mcp_servers: inputs.mcp_servers.as_deref(),
    })?;
    Ok(reg_values(&policy)
        .into_iter()
        .map(|(name, _, value)| (name, value))
        .collect())
}

pub fn render_reg(elevated: bool, inputs: &ProfileGenInputs) -> Result<String, MdmError> {
    Ok(render_reg_values(elevated, &profile_entries(inputs)?))
}
