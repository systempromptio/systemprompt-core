//! Governance for systemprompt.io AI tool calls.
//!
//! Two planes decide whether a principal may act and record why:
//!
//! - Authorization decision plane ([`authz`]) — deny-overrides resolver,
//!   `access_control_rules` repository, and `AuthzDecisionHook` extension
//!   surface shared by the gateway and MCP enforcement sites.
//! - Governed-call plane ([`policy`]) — the `GovernanceEngine` policy chain
//!   (secret scan, scope check, blocklist, rate limit + extension-registered
//!   policies) with per-entry audit tracing into `governance_decisions`.
//!
//! The `authz` member defined here owns the access-control and governance
//! tables (`access_control_*`, `governance_*`, `approval_requests`), their
//! migrations, the two config planes and the built-in `traffic` subject
//! dimension. Its capability slots are in [`contract`].
//!
//! # Feature flags
//!
//! This crate has no Cargo features; everything compiles by default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod authz;
pub mod contract;
pub mod decision_retention;
pub mod policy;

pub use authz::CompositeAuthzHook;
pub use decision_retention::GovernanceDecisionRetention;
