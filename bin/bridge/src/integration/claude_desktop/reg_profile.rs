//! `.reg` serialisation for the Claude Desktop managed-policy profile.
//!
//! The profile *is* the policy: the same
//! [`claude_desktop_policy`] the machine sync enforces, so a repair that
//! a key the sync will then report as different. [`render_reg`] and
//! [`crate::install::reg_values::parse_reg_entries`] are inverses, kept
//! platform-independent so the round-trip is testable on every target.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_models::bridge::profile::AdvertisedLimits;

use super::shared::ProfileGenInputs;
use crate::install::mdm::MdmError;
use crate::install::mdm::policy::{PolicyInputs, claude_desktop_policy, reg_values};
use crate::install::reg_values::render_reg_values;

const ONE_MILLION: u32 = 1_000_000;

// Why: Claude Desktop sizes a gateway model's context from its id, not from
// the gateway's advertised limits: it budgets 200k unless the id carries the
// `[1m]` suffix, which its picker labels "1M context window". Cowork's first
// turn alone is ~198k tokens, so every Claude model the gateway serves at 1M
// is also listed as `<id>[1m]`, right after its bare id. The gateway strips
// the suffix before routing.
#[must_use]
pub fn with_context_variants(
    models: &[String],
    limits: &BTreeMap<String, AdvertisedLimits>,
) -> Vec<String> {
    let mut out = Vec::with_capacity(models.len() * 2);
    for id in models {
        if !out.contains(id) {
            out.push(id.clone());
        }
        let variant = format!("{id}[1m]");
        let is_million = limits
            .get(id)
            .is_some_and(|limit| limit.context_window >= ONE_MILLION);
        if is_million && !id.ends_with("[1m]") && !models.contains(&variant) {
            out.push(variant);
        }
    }
    out
}

pub fn profile_entries(inputs: &ProfileGenInputs) -> Result<Vec<(&'static str, String)>, MdmError> {
    let models = if inputs.models.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&with_context_variants(&inputs.models, &inputs.model_limits))
                .map_err(|e| MdmError::InvalidConfig {
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
