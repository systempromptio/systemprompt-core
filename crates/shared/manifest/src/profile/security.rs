//! Profile `security:` block: signing keys, trusted issuers, resource
//! audiences, UI-session lifetimes, authorization-endpoint policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ClientId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_traits::LinkPolicy;

pub const GATEWAY_REQUIRED_RESOURCE_AUDIENCES: &[&str] = &["hook"];

#[must_use]
pub fn default_resource_audiences() -> Vec<String> {
    GATEWAY_REQUIRED_RESOURCE_AUDIENCES
        .iter()
        .map(|aud| (*aud).to_owned())
        .collect()
}

const fn default_allow_registration() -> bool {
    false
}

pub const DEFAULT_SIGNING_KEY_PATH: &str = "signing_key.pem";

fn default_signing_key_path() -> PathBuf {
    PathBuf::from(DEFAULT_SIGNING_KEY_PATH)
}

pub const DEFAULT_ID_JAG_TTL_SECS: i64 = 300;

const fn default_id_jag_ttl_secs() -> i64 {
    DEFAULT_ID_JAG_TTL_SECS
}

const fn default_ui_session_access_ttl_secs() -> i64 {
    300
}

const fn default_ui_session_absolute_secs() -> i64 {
    12 * 60 * 60
}

const fn default_ui_session_step_up_max_age_secs() -> i64 {
    600
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    #[serde(skip)]
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

    #[serde(default = "default_allow_registration")]
    pub allow_dynamic_client_registration: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_page_url: Option<String>,

    #[serde(default = "default_signing_key_path")]
    pub signing_key_path: PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted_issuers: Vec<TrustedIssuer>,

    #[serde(default = "default_id_jag_ttl_secs")]
    pub id_jag_ttl_secs: i64,

    #[serde(default, skip_serializing_if = "ApiKeyPolicyConfig::is_default")]
    pub api_keys: ApiKeyPolicyConfig,

    #[serde(default, skip_serializing_if = "UiSessionConfig::is_default")]
    pub ui_session: UiSessionConfig,

    #[serde(default, skip_serializing_if = "AuthorizeConfig::is_default")]
    pub authorize: AuthorizeConfig,
}

/// Policy of the authorization endpoint for a person already signed in to the
/// browser (`security.authorize`).
///
/// `/authorize` issues a code without any page when the person holds a browser
/// session and has already allowed the requesting operator-declared client
/// the requested scopes. A client whose `redirect_uri` is a loopback address
/// or a private-use scheme cannot prove its identity (RFC 8252 §8.6): any
/// process on the person's machine can open the same authorization URL and
/// receive the code. For such a redirect the endpoint therefore always shows
/// the one-click consent page, whatever consent is remembered.
/// `silent_native_clients: true` removes that page for native redirects too;
/// the person then has no chance to notice a local process impersonating the
/// client, and PKCE alone decides who can redeem the code.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct AuthorizeConfig {
    #[serde(default)]
    pub silent_native_clients: bool,
}

impl AuthorizeConfig {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Lifetimes, in seconds, of a person's UI session (`security.ui_session`):
/// the bearer token a first-party client the operator declared `person_ui`
/// holds for the person who signed in through it.
///
/// Only the authorization-code + PKCE grant for such a client mints one, and
/// only refreshing that grant's own refresh token renews it. The access token
/// lives `access_ttl_secs`; its `auth_time` is the sign-in, carried unchanged
/// through every refresh until `absolute_secs` after the grant's authorization
/// code was issued, when the session ends whatever the refreshes. A high-impact
/// document action wants a sign-in no older than `step_up_max_age_secs` (RFC
/// 9470).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a duration in seconds; the `_secs` suffix is the profile YAML \
              vocabulary shared with `id_jag_ttl_secs`"
)]
pub struct UiSessionConfig {
    #[serde(default = "default_ui_session_access_ttl_secs")]
    pub access_ttl_secs: i64,

    #[serde(default = "default_ui_session_absolute_secs")]
    pub absolute_secs: i64,

    #[serde(default = "default_ui_session_step_up_max_age_secs")]
    pub step_up_max_age_secs: i64,
}

impl Default for UiSessionConfig {
    fn default() -> Self {
        Self {
            access_ttl_secs: default_ui_session_access_ttl_secs(),
            absolute_secs: default_ui_session_absolute_secs(),
            step_up_max_age_secs: default_ui_session_step_up_max_age_secs(),
        }
    }
}

impl UiSessionConfig {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[must_use]
    pub const fn step_up_required(&self, auth_time: i64, now: i64) -> bool {
        now.saturating_sub(auth_time) > self.step_up_max_age_secs
    }
}

/// The lifetimes, in seconds, of a person's UI session, as the profile's
/// `security.ui_session` block sets them.
///
/// A `person_ui` client reads this capability from either discovery document
/// instead of probing `/authorize`; neither document names a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UiSessionDiscovery {
    pub access_token_lifetime: i64,
    pub session_lifetime: i64,
    pub step_up_max_age: i64,
}

impl UiSessionDiscovery {
    #[must_use]
    pub const fn from_config(config: &UiSessionConfig) -> Self {
        Self {
            access_token_lifetime: config.access_ttl_secs,
            session_lifetime: config.absolute_secs,
            step_up_max_age: config.step_up_max_age_secs,
        }
    }
}

/// Issuance policy for personal access tokens (`security.api_keys`).
///
/// With `max_lifetime_days` set, every issuance path bounds the key: an
/// omitted expiry becomes the maximum and a longer one is refused. Unset
/// leaves keys without a lifetime ceiling.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyPolicyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lifetime_days: Option<u32>,
}

impl ApiKeyPolicyConfig {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A federated identity provider trusted for the RFC 8693 token-exchange and
/// EMA (Enterprise-Managed Authorization) paths.
///
/// `audience` holds the value the `IdP` places in `id_token.aud`; for a
/// Salesforce Connected App that is its `client_id`, **not** a URL. `link`
/// decides whether an identity it asserts may claim an existing account by
/// verified email (off by default) and whether a new one is provisioned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrustedIssuer {
    pub issuer: String,
    pub jwks_uri: String,
    pub audience: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub typ_allowlist: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_client_ids: Vec<ClientId>,

    #[serde(default)]
    pub can_issue_id_jag: bool,

    #[serde(default, skip_serializing_if = "LinkPolicy::is_default")]
    pub link: LinkPolicy,
}
