//! Render the bridge-owned `model` block into Hermes YAML.
//!
//! The generated artifact also carries the `OPENAI_API_KEY` under a private
//! top-level marker so the installer can lift it into `.env` without the
//! `generate` step mutating the real `HERMES_HOME`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::config::{
    API_MODE_VALUE, KEY_ENV_VALUE, MODEL_NAME, MODEL_PROVIDER, PROVIDER_API_MODE,
    PROVIDER_BASE_URL, PROVIDER_ENTRY, PROVIDER_KEY_ENV,
};
use super::super::probe::write_dotted;
use crate::integration::host_app::ProfileGenInputs;

// Why: a top-level key the installer strips before merging, so the static API
// key rides along with the generated profile but never lands in config.yaml.
pub(super) const API_KEY_MARKER: &str = "_systemprompt_openai_api_key";

pub(super) fn managed_yaml(inputs: &ProfileGenInputs) -> std::io::Result<String> {
    let gateway = inputs.gateway_base_url.trim_end_matches('/');

    let mut value = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    // Why: the named `providers:` entry carries the endpoint, and
    // `model.provider` selects it. Both halves are needed — an entry nothing
    // points at is dead config, and a selection with no entry fails resolution.
    write_dotted(
        &mut value,
        MODEL_PROVIDER,
        serde_yaml::Value::String(PROVIDER_ENTRY.to_owned()),
    );
    write_dotted(
        &mut value,
        PROVIDER_BASE_URL,
        serde_yaml::Value::String(format!("{gateway}/v1")),
    );
    write_dotted(
        &mut value,
        PROVIDER_API_MODE,
        serde_yaml::Value::String(API_MODE_VALUE.to_owned()),
    );
    write_dotted(
        &mut value,
        PROVIDER_KEY_ENV,
        serde_yaml::Value::String(KEY_ENV_VALUE.to_owned()),
    );
    if let Some(model) = inputs.models.first() {
        write_dotted(
            &mut value,
            MODEL_NAME,
            serde_yaml::Value::String(model.clone()),
        );
    }
    write_dotted(
        &mut value,
        API_KEY_MARKER,
        serde_yaml::Value::String(inputs.api_key.clone()),
    );

    serde_yaml::to_string(&value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
