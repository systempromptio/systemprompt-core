//! Governance configuration for the gateway + MCP authorization hook.
//!
//! Authz is **fail-closed** with an explicit-opt-in surface. Three modes:
//!
//! - `webhook` — production. Core POSTs every request to the configured URL;
//!   any transport error, non-2xx, or decode failure denies the request.
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
//! Example:
//!
//! ```yaml
//! governance:
//!   authz:
//!     hook:
//!       mode: webhook
//!       url: http://localhost:8080/api/public/govern/authz
//!       timeout_ms: 500
//! ```

use serde::{Deserialize, Serialize};

pub const UNRESTRICTED_ACKNOWLEDGEMENT: &str = "I understand this disables all authorization";

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernanceConfig {
    #[serde(default)]
    pub authz: Option<AuthzConfig>,
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
    Disabled,
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthzHookConfig {
    pub mode: AuthzMode,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub acknowledgement: Option<String>,
}

const fn default_timeout_ms() -> u64 {
    500
}
