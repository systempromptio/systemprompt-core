//! The managed inference block: the provider, loopback endpoint and model list
//! Cowork reads from policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const DEFAULT_INFERENCE_MODELS: &[&str] = &["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"];

#[must_use]
pub fn default_inference_models() -> Vec<String> {
    DEFAULT_INFERENCE_MODELS
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}
