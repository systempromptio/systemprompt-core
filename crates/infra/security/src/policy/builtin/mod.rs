//! The built-in governance policies.
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
//! A fifth, `require_approval`, is registered but not enabled by `defaults` —
//! it is the only policy that returns
//! [`Decision::Pending`][crate::authz::Decision] rather than an allow or a
//! deny, and it holds nothing until it is given patterns to match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rate_limit;
mod require_approval;
mod scope_check;
mod secret_scan;
mod tool_blocklist;

pub use require_approval::ApprovalSettings;
