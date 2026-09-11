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

    // Why: OpenCode has no models.dev catalogue for custom providers, so models
    // must be declared.
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
    // Why: with the whole catalog advertised, the first entry is whichever
    // provider happens to sort first — not a choice. Prefer the gateway's own
    // default when it is one of the models we just declared, since a default
    // OpenCode cannot resolve leaves the picker broken on first launch.
    let default = inputs
        .default_model
        .as_ref()
        .filter(|m| inputs.models.contains(m))
        .or_else(|| inputs.models.first());
    if let Some(model) = default {
        root.insert(
            DEFAULT_MODEL.to_owned(),
            json!(format!("{PROVIDER_ID}/{model}")),
        );
    }
    root.insert(API_KEY_MARKER.to_owned(), json!(inputs.api_key));
    root
}
