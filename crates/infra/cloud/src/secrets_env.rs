//! Deploy-time mapping of a profile's `secrets.json` to environment variables.
//!
//! [`load_secrets_json`] reads the profile's secrets file,
//! [`map_secrets_to_env_vars`] translates the well-known keys to their
//! provider environment-variable names (dropping system-managed keys and
//! advertising custom ones via `CUSTOM_SECRETS`), and
//! [`read_signing_key_pem`] base64-encodes the JWT signing key for transport
//! as the `SIGNING_KEY_PEM` secret.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine;

use crate::constants::env_vars;
use crate::error::{CloudError, CloudResult};

pub fn load_secrets_json(path: &Path) -> CloudResult<HashMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CloudError::deploy_with(format!("Failed to read {}", path.display()), e))?;

    // JSON: free-form user-authored file; non-string entries are skipped, not
    // rejected.
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| CloudError::deploy_with("Failed to parse secrets.json", e))?;

    let mut secrets = HashMap::new();

    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str()
                && !s.is_empty()
            {
                secrets.insert(key.clone(), s.to_owned());
            }
        }
    }

    Ok(secrets)
}

#[must_use]
pub fn map_secrets_to_env_vars<S: std::hash::BuildHasher>(
    secrets: HashMap<String, String, S>,
) -> HashMap<String, String> {
    let has_internal = secrets.contains_key("internal_database_url");

    let mut result: HashMap<String, String> = secrets
        .into_iter()
        .filter_map(|(k, v)| {
            let env_key = to_env_var_name(&k, has_internal)?;
            if env_vars::is_system_managed(&env_key) {
                tracing::warn!(key = %env_key, "Skipping system-managed variable from secrets.json");
                return None;
            }
            Some((env_key, v))
        })
        .collect();

    let custom_keys: Vec<String> = result
        .keys()
        .filter(|k| !is_standard_env_var(k))
        .cloned()
        .collect();

    if !custom_keys.is_empty() {
        result.insert(env_vars::CUSTOM_SECRETS.to_owned(), custom_keys.join(","));
    }

    result
}

fn to_env_var_name(key: &str, has_internal_db_url: bool) -> Option<String> {
    match key {
        "gemini" => Some("GEMINI_API_KEY".to_owned()),
        "anthropic" => Some("ANTHROPIC_API_KEY".to_owned()),
        "openai" => Some("OPENAI_API_KEY".to_owned()),
        "internal_database_url" => Some("DATABASE_URL".to_owned()),
        "database_url" if has_internal_db_url => None,
        _ => Some(key.to_uppercase()),
    }
}

fn is_standard_env_var(key: &str) -> bool {
    matches!(
        key,
        "OAUTH_AT_REST_PEPPER"
            | "DATABASE_URL"
            | "GEMINI_API_KEY"
            | "ANTHROPIC_API_KEY"
            | "OPENAI_API_KEY"
            | "GITHUB_TOKEN"
    )
}

pub fn read_signing_key_pem(path: &Path) -> CloudResult<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let pem = std::fs::read_to_string(path).map_err(|e| {
        CloudError::deploy_with(format!("reading signing key {}", path.display()), e)
    })?;
    Ok(Some(
        base64::engine::general_purpose::STANDARD.encode(pem.as_bytes()),
    ))
}
