//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

/// The resource audiences every generated profile must opt into.
///
/// Returns [`GATEWAY_REQUIRED_RESOURCE_AUDIENCES`] as owned strings, so the
/// setup wizard and the env-driven cloud bootstrap seed the same audiences and
/// pass [`crate::profile::Profile::validate`] from one source of truth.
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

/// Default ID-JAG lifetime in seconds; short by design (draft §6) to bound
/// replay.
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

    /// Accepted JOSE `typ` header values; empty accepts any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typ_allowlist: Vec<String>,

    /// `client_id`/`azp` values accepted on the EMA consume path; empty accepts
    /// any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_client_ids: Vec<String>,

    /// Whether this issuer's `id_token` may seed the EMA ID-JAG issuance path.
    #[serde(default)]
    pub can_issue_id_jag: bool,
}
