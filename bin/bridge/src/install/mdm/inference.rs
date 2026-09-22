//! The managed inference block: the provider, loopback endpoint, per-host
//! token and model list Cowork reads from policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::MdmError;
use super::policy::{PolicyEntry, PolicyInputs, PolicyValue};

const DEFAULT_INFERENCE_MODELS: &[&str] = &[
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5",
    "claude-fable-5-1",
    "claude-haiku-4-5-20251001",
];

pub(super) const INFERENCE_MODELS_KEY: &str = "inferenceModels";

#[must_use]
pub fn default_inference_models() -> Vec<String> {
    DEFAULT_INFERENCE_MODELS
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

// Why: Claude Desktop breaks when `inferenceModels` names a non-Anthropic
// family, so every list is filtered to Claude ids at the one place the key is
// built. Desktop-only by construction (all callers go through
// `claude_desktop_policy`); it is the single carve-out from the reachability
// rule — every other host gets the whole advertised catalog, since the gateway
// transcodes every inbound wire to every provider wire.
fn anthropic_only(models: Vec<String>) -> Vec<String> {
    let kept: Vec<String> = models
        .into_iter()
        .filter(|id| {
            let lower = id.to_ascii_lowercase();
            lower.contains("claude") || lower.contains("anthropic")
        })
        .collect();
    if kept.is_empty() {
        default_inference_models()
    } else {
        kept
    }
}

fn configured_models(raw: Option<&str>) -> Result<Vec<String>, MdmError> {
    let Some(raw) = raw.map(str::trim).filter(|m| !m.is_empty()) else {
        return Ok(default_inference_models());
    };
    serde_json::from_str::<Vec<String>>(raw).map_err(|e| MdmError::InvalidConfig {
        key: INFERENCE_MODELS_KEY,
        detail: e.to_string(),
    })
}

pub(super) fn inference_entries(inputs: &PolicyInputs<'_>) -> Result<Vec<PolicyEntry>, MdmError> {
    let models = anthropic_only(configured_models(inputs.models.as_deref())?);
    Ok(vec![
        ("inferenceProvider", PolicyValue::Str("gateway".into())),
        (
            "inferenceGatewayBaseUrl",
            PolicyValue::Str(inputs.base_url.to_owned()),
        ),
        (
            crate::cowork_compat::POLICY_API_KEY,
            PolicyValue::Str(inputs.host_token.as_str().to_owned()),
        ),
        (
            "inferenceGatewayAuthScheme",
            PolicyValue::Str("bearer".into()),
        ),
        (
            INFERENCE_MODELS_KEY,
            PolicyValue::Json(super::policy::json_of(&models)),
        ),
    ])
}
