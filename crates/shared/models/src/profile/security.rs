//! Profile `security:` block: signing keys, trusted issuers, resource
//! audiences.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use crate::auth::JwtAudience;
use serde::{Deserialize, Serialize};

pub const GATEWAY_REQUIRED_RESOURCE_AUDIENCES: &[&str] = &["hook"];

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

pub const DEFAULT_ID_JAG_TTL_SECS: i64 = 300;

const fn default_id_jag_ttl_secs() -> i64 {
    DEFAULT_ID_JAG_TTL_SECS
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

    // Why: when set, the OAuth authorize endpoint 302s to this
    // deployment-owned sign-in page (carrying the original query) instead of
    // rendering the built-in WebAuthn form; prompt=passkey opts back in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_page_url: Option<String>,

    #[serde(default = "default_signing_key_path")]
    pub signing_key_path: PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_issuers: Vec<TrustedIssuer>,

    #[serde(default = "default_id_jag_ttl_secs")]
    pub id_jag_ttl_secs: i64,
}

/// A federated identity provider trusted for the RFC 8693 token-exchange and
/// EMA (Enterprise-Managed Authorization) paths.
///
/// `audience` holds the value the `IdP` places in `id_token.aud`; for a
/// Salesforce Connected App that is its `client_id`, **not** a URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustedIssuer {
    pub issuer: String,
    pub jwks_uri: String,
    pub audience: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typ_allowlist: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_client_ids: Vec<String>,

    #[serde(default)]
    pub can_issue_id_jag: bool,
}
