use std::path::PathBuf;

use crate::auth::JwtAudience;
use serde::{Deserialize, Serialize};

/// Audiences the gateway's grant paths require to be present in
/// [`SecurityConfig::allowed_resource_audiences`].
///
/// These are not RFC 8707 external resource URIs — they are the gateway's own
/// internal protocol audiences that hardcoded scope guards depend on. The
/// `client_credentials` grant rejects any `hook:*` scope that is not paired
/// with `audience=hook`, so a profile that does not opt into the `"hook"`
/// audience cannot mint plugin hook tokens for the bridge. Profile validation
/// rejects bootstrap if any entry here is missing, so the error surfaces at
/// the operator's YAML edit rather than at a downstream tenant's first call.
pub const GATEWAY_REQUIRED_RESOURCE_AUDIENCES: &[&str] = &["hook"];

/// The resource audiences every generated profile must opt into so it passes
/// [`crate::profile::Profile::validate`] — the single source of truth shared by
/// the setup wizard and the env-driven cloud bootstrap.
#[must_use]
pub fn default_resource_audiences() -> Vec<String> {
    GATEWAY_REQUIRED_RESOURCE_AUDIENCES
        .iter()
        .map(|aud| (*aud).to_owned())
        .collect()
}

const fn default_allow_registration() -> bool {
    true
}

fn default_signing_key_path() -> PathBuf {
    PathBuf::from("signing_key.pem")
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    #[serde(rename = "jwt_issuer")]
    pub issuer: String,

    #[serde(rename = "jwt_access_token_expiration")]
    pub access_token_expiration: i64,

    #[serde(rename = "jwt_refresh_token_expiration")]
    pub refresh_token_expiration: i64,

    #[serde(rename = "jwt_audiences")]
    pub audiences: Vec<JwtAudience>,

    #[serde(default)]
    pub allowed_resource_audiences: Vec<String>,

    #[serde(default = "default_allow_registration")]
    pub allow_registration: bool,

    #[serde(default = "default_signing_key_path")]
    pub signing_key_path: PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_issuers: Vec<TrustedIssuer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustedIssuer {
    pub issuer: String,
    pub jwks_uri: String,
    pub audience: String,
}
