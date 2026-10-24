//! The managed inference block: the provider, loopback endpoint, per-host
//! token and model list Cowork reads from policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::MdmError;
use super::policy::{PolicyEntry, PolicyInputs, PolicyValue};

const DEFAULT_INFERENCE_MODELS: &[&str] = &["claude-sonnet-5[1m]"];

pub(super) const INFERENCE_MODELS_KEY: &str = "inferenceModels";

#[must_use]
pub fn default_inference_models() -> Vec<String> {
    DEFAULT_INFERENCE_MODELS
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

fn configured_models(raw: Option<&str>) -> Result<Vec<String>, MdmError> {
    let Some(raw) = raw.map(str::trim).filter(|m| !m.is_empty()) else {
        return Ok(default_inference_models());
    };
    serde_json::from_str::<Vec<String>>(raw).map_err(|source| MdmError::ConfigJson {
        key: INFERENCE_MODELS_KEY,
        source,
    })
}

// Why: the list arrives already narrowed to the models the catalog
// advertises to Claude clients (`model_view::host_model_view`); Claude
// Desktop rejects any other id in `inferenceModels`, and the catalog's
// `clients` is the one rule that decides it.
pub(super) fn inference_entries(inputs: &PolicyInputs<'_>) -> Result<Vec<PolicyEntry>, MdmError> {
    let models = configured_models(inputs.models.as_deref())?;
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
