//! The managed inference block: the provider, loopback endpoint and model list
//! Cowork reads from policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: the models the managed gateway block advertises when the policy has none
// yet. It lives here, with the block that writes it, rather than in the Claude
// Desktop host — `install` sits below `integration`, so the host reads it from
// here and not the other way round.
const DEFAULT_INFERENCE_MODELS: &[&str] = &["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];

#[must_use]
pub fn default_inference_models() -> Vec<String> {
    DEFAULT_INFERENCE_MODELS
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

// Why: Cowork treats `inferenceProvider=gateway` without a base URL and a
// credential as an unusable configuration and refuses to start any task, so
// the gateway block is written as one unit or not at all. The URL and secret
// are the loopback proxy's, so a rotated secret self-heals on the next sync;
// an `inferenceModels` value already on the machine (the host profile writes
// the gateway's compatible list) is kept over the default.
#[must_use]
pub fn inference_policy_values(
    base_url: &str,
    api_key: &str,
    existing_models: Option<String>,
) -> Vec<(&'static str, &'static str, String)> {
    let models = existing_models
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| {
            serde_json::to_string(&default_inference_models()).unwrap_or_else(|_| "[]".into())
        });
    vec![
        ("inferenceProvider", "REG_SZ", "gateway".into()),
        ("inferenceGatewayBaseUrl", "REG_SZ", base_url.to_owned()),
        ("inferenceGatewayApiKey", "REG_SZ", api_key.to_owned()),
        ("inferenceGatewayAuthScheme", "REG_SZ", "bearer".into()),
        ("inferenceModels", "REG_SZ", models),
    ]
}

// Why: a sync that cannot read the loopback secret must not write a gateway
// block with no credential — that half-written policy is exactly what makes
// Cowork refuse to start tasks — so it fails and leaves the current one alone.
#[cfg(target_os = "windows")]
pub(super) fn inference_values(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<Vec<(&'static str, &'static str, String)>, super::MdmError> {
    let secret = inputs.loopback.secret().map_err(|e| {
        super::MdmError::Windows(format!(
            "loopback secret unavailable ({e}); the gateway policy block was not written. Start \
             the Bridge proxy, then sync again."
        ))
    })?;
    let existing_models = crate::config::store::managed_policy_store()
        .read_managed_policy("inferenceModels")
        .ok()
        .flatten();
    Ok(inference_policy_values(
        &inputs.loopback.origin(),
        secret.as_str(),
        existing_models,
    ))
}
