use std::path::PathBuf;

use crate::auth::JwtAudience;
use serde::{Deserialize, Serialize};

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
