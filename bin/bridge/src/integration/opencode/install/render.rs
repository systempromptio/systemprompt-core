//! Render the bridge-owned provider block into `OpenCode` JSON.
//!
//! The generated artifact also carries the API key under a private top-level
//! marker so the installer can lift it into `auth.json` without the
//! `generate` step touching the user's data directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value, json};

use super::super::config::{DEFAULT_MODEL, NPM_PACKAGE, PROVIDER_ID};
use crate::integration::host_app::ProfileGenInputs;

// Why: a top-level key the installer strips before merging, so the static API
// key rides along with the generated profile but never lands in the managed
// file.
pub(super) const API_KEY_MARKER: &str = "_systemprompt_api_key";

pub(super) fn managed_json_text(inputs: &ProfileGenInputs) -> std::io::Result<String> {
    let value = managed_json(inputs);
    serde_json::to_string_pretty(&Value::Object(value))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub(super) fn managed_json(inputs: &ProfileGenInputs) -> Map<String, Value> {
    let gateway = inputs.gateway_base_url.trim_end_matches('/');

    let mut options = Map::new();
    options.insert("baseURL".to_owned(), json!(format!("{gateway}/v1")));
    if !inputs.headers.is_empty() {
        let headers: Map<String, Value> = inputs
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        options.insert("headers".to_owned(), Value::Object(headers));
    }

    // Why: a custom provider has no models.dev catalogue, so every compatible
    // model must be declared or OpenCode offers nothing under it.
    let models: Map<String, Value> = inputs
        .models
        .iter()
        .map(|m| (m.clone(), json!({ "name": m })))
        .collect();

    let mut provider = Map::new();
    provider.insert("npm".to_owned(), json!(NPM_PACKAGE));
    provider.insert("name".to_owned(), json!("systemprompt.io gateway"));
    provider.insert("options".to_owned(), Value::Object(options));
    provider.insert("models".to_owned(), Value::Object(models));

    let mut providers = Map::new();
    providers.insert(PROVIDER_ID.to_owned(), Value::Object(provider));

    let mut root = Map::new();
    root.insert("provider".to_owned(), Value::Object(providers));
    if let Some(first) = inputs.models.first() {
        root.insert(
            DEFAULT_MODEL.to_owned(),
            json!(format!("{PROVIDER_ID}/{first}")),
        );
    }
    root.insert(API_KEY_MARKER.to_owned(), json!(inputs.api_key));
    root
}
