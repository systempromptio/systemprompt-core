//! Diagnostic helpers for secrets-loading failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::secrets::Secrets;

use crate::error::ConfigError;

pub(super) fn log_secrets_failure(e: &ConfigError) {
    tracing::error!(error = %e, "secrets could not be loaded");
}

#[must_use]
pub fn build_loaded_secrets_message(secrets: &Secrets) -> String {
    let base = ["oauth_at_rest_pepper", "database_url"];
    let optional_providers = [
        secrets
            .database_write_url
            .as_ref()
            .map(|_| "database_write_url"),
        secrets
            .external_database_url
            .as_ref()
            .map(|_| "external_database_url"),
        secrets
            .internal_database_url
            .as_ref()
            .map(|_| "internal_database_url"),
        secrets.gemini.as_ref().map(|_| "gemini"),
        secrets.anthropic.as_ref().map(|_| "anthropic"),
        secrets.openai.as_ref().map(|_| "openai"),
        secrets.github.as_ref().map(|_| "github"),
    ];

    let loaded: Vec<&str> = base
        .into_iter()
        .chain(optional_providers.into_iter().flatten())
        .collect();

    if secrets.custom.is_empty() {
        format!("Loaded secrets: {}", loaded.join(", "))
    } else {
        format!(
            "Loaded secrets: {}, {} custom",
            loaded.join(", "),
            secrets.custom.len()
        )
    }
}
