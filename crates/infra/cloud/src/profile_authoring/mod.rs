//! Profile construction for local and cloud deployment targets.
//!
//! [`LocalProfileBuilder`] and [`CloudProfileBuilder`] encode the differing
//! defaults for each target (host, URLs, paths, security, runtime, and
//! validation modes) so callers only supply the tenant-specific fields.
//! Construction is pure — no prompting and no filesystem writes; interactive
//! collection of the inputs stays in the CLI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cloud_builder;
mod local_builder;

pub use cloud_builder::CloudProfileBuilder;
pub use local_builder::LocalProfileBuilder;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::profile::{
    AuthzConfig, AuthzHookConfig, AuthzMode, GovernanceConfig, TrustedIssuer,
    default_resource_audiences,
};
use systemprompt_models::{Environment, LogLevel, OutputFormat, RuntimeConfig, SecurityConfig};

use crate::constants::profile as consts;

#[must_use]
pub fn generate_display_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "dev" | "development" => "Development".to_owned(),
        "prod" | "production" => "Production".to_owned(),
        "staging" | "stage" => "Staging".to_owned(),
        "test" | "testing" => "Test".to_owned(),
        "local" => "Local Development".to_owned(),
        "cloud" => "Cloud".to_owned(),
        _ => capitalize_first(name),
    }
}

fn capitalize_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn webhook_governance(api_internal_url: &str) -> GovernanceConfig {
    GovernanceConfig {
        authz: Some(AuthzConfig {
            hook: AuthzHookConfig {
                mode: AuthzMode::Webhook,
                url: Some(format!("{api_internal_url}/api/public/govern/authz")),
                timeout_ms: 500,
                acknowledgement: None,
            },
        }),
    }
}

fn security_config(issuer: &str, trusted_issuers: Vec<TrustedIssuer>) -> SecurityConfig {
    SecurityConfig {
        issuer: issuer.to_owned(),
        access_token_expiration: consts::ACCESS_TOKEN_EXPIRATION,
        refresh_token_expiration: consts::REFRESH_TOKEN_EXPIRATION,
        audiences: JwtAudience::standard(),
        allowed_resource_audiences: default_resource_audiences(),
        allow_registration: true,
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        trusted_issuers,
        id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
    }
}

const fn local_runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        environment: Environment::Development,
        log_level: LogLevel::Verbose,
        output_format: OutputFormat::Text,
        no_color: false,
        non_interactive: false,
    }
}

const fn cloud_runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        environment: Environment::Production,
        log_level: LogLevel::Normal,
        output_format: OutputFormat::Json,
        no_color: true,
        non_interactive: true,
    }
}
