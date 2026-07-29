//! Unified tool-use governance plane.
//!
//! This module defines the shared types and trait that every governance policy
//! in the system implements. A governed call is an MCP tool invocation or a
//! submitted prompt ([`GovernedTarget`]), carrying tool arguments or prompt
//! text ([`GovernedInput`]). It is consumed by the template's policy chain
//! (secret scan, scope check, blocklist, rate limit) and produces the same
//! typed [`crate::authz::types::Decision`] the user→entity resolver returns —
//! so a single audit shape and a single CLI view cover both planes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod governed;
pub mod types;

pub use governed::{
    GovernedInput, GovernedString, GovernedTarget, McpToolInput, PROMPT_TARGET_NAME,
    UNKNOWN_TARGET_NAME,
};
pub use types::{
    AgentScope, GovernanceChain, GovernancePolicy, PolicyContext, RateLimitWindow, SecretLocation,
};
