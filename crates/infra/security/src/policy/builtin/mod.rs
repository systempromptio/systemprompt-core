//! The four built-in governance policies.
//!
//! Each registers itself with the [`super::registry`] under a stable id and is
//! enabled by [`super::GovernanceConfig::defaults`]:
//!
//! | id | denies with |
//! |----|-------------|
//! | `secret_scan` | [`DenyReason::SecretLeak`][crate::authz::DenyReason::SecretLeak] |
//! | `scope_check` | [`DenyReason::ScopeViolation`][crate::authz::DenyReason::ScopeViolation] |
//! | `tool_blocklist` | [`DenyReason::ToolBlocked`][crate::authz::DenyReason::ToolBlocked] |
//! | `rate_limit` | [`DenyReason::RateLimitExceeded`][crate::authz::DenyReason::RateLimitExceeded] |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rate_limit;
mod scope_check;
mod secret_scan;
mod tool_blocklist;
