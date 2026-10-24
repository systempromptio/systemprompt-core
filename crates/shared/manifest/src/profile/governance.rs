//! Governance configuration for the gateway + MCP authorization hook.
//!
//! Authz is **fail-closed** with an explicit-opt-in surface. Four modes:
//!
//! - `webhook` — production. Core POSTs every request to the server's own
//!   `/api/public/govern/authz`, derived from `server.host`/`server.port`; any
//!   transport error, non-2xx, or decode failure denies the request.
//! - `extension` — production, explicit opt-in. The hook is supplied at
//!   bootstrap by the binary via `register_authz_hook!` or
//!   `AppContextBuilder::with_authz_hook(...)`. Bootstrap errors if no hook is
//!   supplied. See `internal/guides/authz.md`.
//!
//! When an extension hook is registered and the profile says `webhook`, the
//! registered hook is used, `url` is ignored, and one warning asks for
//! `mode: extension`. A hook registered under `disabled` or `unrestricted`
//! still refuses to boot.
//! - `disabled` — denies every request via `DenyAllHook`. Use when authz is
//!   intentionally inactive but you want the surface installed.
//! - `unrestricted` — TEST/DEV ONLY. Allows every request via `AllowAllHook`.
//!   Requires `acknowledgement` to equal the literal sentence `"I understand
//!   this disables all authorization"`. Bootstrap errors otherwise.
//!
//! Absent `governance` block, absent `authz`, or any unparseable config →
//! bootstrap installs `DenyAllHook` (everything denied) so misconfiguration
//! never silently grants access.
//!
//! `audit` bounds what the gateway retains per request. A body at or under
//! `payload_cap_bytes` is stored whole in `ai_request_payloads`; over it, only
//! the SHA-256 digest and a head+tail excerpt survive. The digest always covers
//! the full bytes, so a capped capture still proves which body was sent.
//!
//! Example:
//!
//! ```yaml
//! governance:
//!   authz:
//!     hook:
//!       mode: webhook
//!       timeout_ms: 500
//!   audit:
//!     payload_cap_bytes: 4194304
//!   quota:
//!     shared_subject_shards: 8
//!   unapplied_planes: warn   # or deny
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const UNRESTRICTED_ACKNOWLEDGEMENT: &str = "I understand this disables all authorization";

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernanceConfig {
    #[serde(default)]
    pub authz: Option<AuthzConfig>,
    #[serde(default)]
    pub audit: AuditConfig,
    #[serde(default)]
    pub quota: QuotaConfig,
    #[serde(default)]
    pub unapplied_planes: UnappliedPlaneMode,
}

/// What a node does while a config plane has never been applied.
///
/// Covers `governance` and `gateway_policies` until `systemprompt admin
/// config apply` records them: `warn` lets requests through and warns at
/// boot, in `doctor`, `/readyz` and on every request; `deny` refuses every
/// governed and gateway request.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum UnappliedPlaneMode {
    #[default]
    Warn,
    Deny,
}

impl UnappliedPlaneMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Deny => "deny",
        }
    }

    #[must_use]
    pub const fn denies(self) -> bool {
        matches!(self, Self::Deny)
    }
}

/// Row sharding for gateway quota windows.
///
/// A window keyed by a shared subject (anything other than `user` or
/// `api_key`, e.g. an organization or project) is spread over
/// `shared_subject_shards` rows so concurrent admissions do not queue on one
/// row lock; per-principal windows use `principal_subject_shards`. Admission
/// still checks the summed total, but each extra shard widens the
/// over-admission bound by up to one concurrent reservation. `1` keeps the
/// exact single-row behaviour. Both are range-checked by
/// [`crate::profile::Profile::validate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuotaConfig {
    #[serde(default = "default_shared_subject_shards")]
    pub shared_subject_shards: u16,
    #[serde(default = "default_principal_subject_shards")]
    pub principal_subject_shards: u16,
}

impl QuotaConfig {
    pub const DEFAULT_SHARED_SUBJECT_SHARDS: u16 = 8;
    pub const DEFAULT_PRINCIPAL_SUBJECT_SHARDS: u16 = 1;
    pub const MAX_SHARDS: u16 = 64;
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            shared_subject_shards: Self::DEFAULT_SHARED_SUBJECT_SHARDS,
            principal_subject_shards: Self::DEFAULT_PRINCIPAL_SUBJECT_SHARDS,
        }
    }
}

const fn default_shared_subject_shards() -> u16 {
    QuotaConfig::DEFAULT_SHARED_SUBJECT_SHARDS
}

const fn default_principal_subject_shards() -> u16 {
    QuotaConfig::DEFAULT_PRINCIPAL_SUBJECT_SHARDS
}

/// Retention bounds for the gateway audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditConfig {
    #[serde(default = "default_payload_cap_bytes")]
    pub payload_cap_bytes: usize,
}

impl AuditConfig {
    pub const DEFAULT_PAYLOAD_CAP_BYTES: usize = 1024 * 1024;
    pub const MIN_PAYLOAD_CAP_BYTES: usize = 64 * 1024;
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            payload_cap_bytes: Self::DEFAULT_PAYLOAD_CAP_BYTES,
        }
    }
}

const fn default_payload_cap_bytes() -> usize {
    AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthzConfig {
    pub hook: AuthzHookConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AuthzMode {
    Webhook,
    Extension,
    Disabled,
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthzHookConfig {
    pub mode: AuthzMode,
    #[serde(skip)]
    pub url: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub acknowledgement: Option<String>,
}

const fn default_timeout_ms() -> u64 {
    500
}
