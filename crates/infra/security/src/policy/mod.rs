//! Unified tool-use governance plane.
//!
//! This module defines the shared types and trait that every tool-call
//! governance policy in the system implements. It is consumed by the
//! template's policy chain (secret scan, scope check, blocklist, rate limit)
//! and produces the same typed [`crate::authz::types::Decision`] the
//! user→entity resolver returns — so a single audit shape and a single CLI view
//! cover both planes.

pub mod types;

pub use types::{
    AgentScope, GovernanceChain, GovernancePolicy, McpToolInput, PolicyContext, RateLimitWindow,
    SecretLocation,
};
