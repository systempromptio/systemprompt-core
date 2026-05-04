//! Diagnostic helpers for secrets-loading failures.

use systemprompt_models::profile::SecretsValidationMode;
use systemprompt_models::secrets::Secrets;

use crate::error::ConfigError;

/// Log a secrets-load failure at the severity selected by `mode`.
///
/// `Strict` mode is silent because the error is propagated to the
/// caller; `Warn` and `Skip` modes emit at warn / debug respectively.
pub fn log_secrets_issue(e: &ConfigError, mode: SecretsValidationMode) {
    match mode {
        SecretsValidationMode::Warn => log_secrets_warn(e),
        SecretsValidationMode::Skip => log_secrets_skip(e),
        SecretsValidationMode::Strict => {},
    }
}

/// Log a secrets-load failure at warning severity.
pub fn log_secrets_warn(e: &ConfigError) {
    tracing::warn!("Secrets file issue: {}", e);
}

/// Log a secrets-load failure at debug severity.
pub fn log_secrets_skip(e: &ConfigError) {
    tracing::debug!("Skipping secrets file: {}", e);
}

/// Build the human-readable summary logged after secrets are loaded
/// successfully.
#[must_use]
pub fn build_loaded_secrets_message(secrets: &Secrets) -> String {
    let base = ["jwt_secret", "database_url"];
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
