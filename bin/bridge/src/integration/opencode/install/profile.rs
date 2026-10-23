//! Render the bridge's V2 `opencode.json` provider block with the shared
//! builder; the host token is named by `{file:}` and written beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_models::bridge::opencode::{
    ApiKeyRef, ModelEntry, ModelLimit, OpenCodeConfig, Policy, PolicyAction, gateway_provider,
};

use super::super::config::{self, PROVIDER_ID};
use crate::integration::host_app::ProfileGenInputs;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct GeneratedOpenCode {
    pub token: String,
    // JSON: the rendered V2 document is merged into a foreign config
    pub config: serde_json::Value,
}

pub(in crate::integration::opencode) fn provider_policies() -> Vec<Policy> {
    vec![
        Policy::deny(PolicyAction::Provider, "*"),
        Policy::allow(PolicyAction::Provider, PROVIDER_ID),
    ]
}

pub(in crate::integration::opencode) fn render(inputs: &ProfileGenInputs) -> OpenCodeConfig {
    // Why: OpenCode has no catalogue for a custom provider, so models are
    // declared, limits included, or it sizes a 1M model with its own default.
    let models: BTreeMap<String, ModelEntry> = inputs
        .models
        .iter()
        .map(|model| {
            let limit = inputs.model_limits.get(model).map(|l| ModelLimit {
                context: u64::from(l.context_window),
                output: u64::from(l.max_output_tokens),
            });
            (
                model.clone(),
                ModelEntry {
                    model_id: model.clone(),
                    limit,
                },
            )
        })
        .collect();
    let api_key = ApiKeyRef::File(config::token_path().display().to_string());
    let provider = gateway_provider(
        &inputs.gateway_base_url,
        &api_key,
        inputs.headers.clone(),
        models,
    );
    let mut rendered = OpenCodeConfig {
        update: None,
        share: None,
        ..OpenCodeConfig::default()
    }
    .with_provider(PROVIDER_ID, provider)
    .with_policies(provider_policies());
    // Why: a `[1m]` default is Claude Code's context marker, matched here as
    // its catalogue id; a default OpenCode cannot resolve breaks the picker.
    let default = inputs
        .default_model
        .as_deref()
        .map(systemprompt_models::providers::without_context_variant)
        .and_then(|d| inputs.models.iter().find(|m| m.as_str() == d))
        .or_else(|| inputs.models.first());
    if let Some(model) = default {
        rendered = rendered.with_model(PROVIDER_ID, model);
    }
    rendered
}

pub(in crate::integration::opencode) fn generated_text(
    inputs: &ProfileGenInputs,
) -> std::io::Result<String> {
    let config = serde_json::to_value(render(inputs))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    serde_json::to_string_pretty(&GeneratedOpenCode {
        token: inputs.host_token.as_str().to_owned(),
        config,
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
