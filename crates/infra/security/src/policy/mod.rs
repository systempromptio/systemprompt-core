//! Unified tool-use governance plane.
//!
//! A governed call is an MCP tool invocation or a submitted prompt
//! ([`GovernedTarget`]), carrying tool arguments or prompt text
//! ([`GovernedInput`]). This module owns the whole enforcement runtime:
//!
//! - [`types`] — the [`GovernancePolicy`] trait and [`PolicyContext`].
//! - [`config`] — the `governance.policies` YAML declaration
//!   ([`GovernanceConfig`]).
//! - [`registry`] — inventory registration
//!   ([`crate::register_governance_policy!`]); the [`builtin`] policies (secret
//!   scan, scope check, blocklist, rate limit, require approval) self-register
//!   here.
//! - [`engine`] — [`GovernanceEngine`], the traced first-halt-wins evaluator.
//! - [`audit`] — [`DecisionAudit`], the typed blob persisted through
//!   [`record_decision`] into `governance_decisions`.
//! - [`secrets`] — the shared credential scanner.
//!
//! Decisions are the same typed [`crate::authz::types::Decision`] the
//! user→entity resolver returns, so a single audit shape and a single CLI
//! view cover both planes. Enforcement points (webhook handlers, in-process
//! seams) live in extensions; they resolve identity and scope, build a
//! [`PolicyContext`], and call the engine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod approval;
pub mod audit;
pub mod builtin;
pub mod config;
pub mod engine;
pub mod governed;
pub mod registry;
pub mod secrets;
pub mod types;

pub use approval::{
    ApprovalOutcome, ApprovalRepository, ApprovalRequest, ApprovalStatus, ApprovalVerdict,
    NewApprovalRequest, args_digest, wait_for_decision,
};
pub use audit::{
    ApproverStamp, AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, ClaimedAgent,
    DecisionAudit, PrincipalSnapshot, record_decision,
};
pub use builtin::ApprovalSettings;
pub use config::{GovernanceConfig, GovernanceConfigError, PolicyConfig};
pub use engine::{Evaluation, GovernanceEngine};
pub use governed::{
    GovernedInput, GovernedString, GovernedTarget, McpToolInput, PROMPT_TARGET_NAME, PromptPart,
    UNKNOWN_TARGET_NAME,
};
pub use registry::{PolicyFactory, PolicyRegistration};
pub use secrets::{EntropyConfig, detect_secrets, detect_secrets_with, scan_str_for_secret};
pub use types::{AgentScope, GovernancePolicy, PolicyContext, RateLimitWindow, SecretLocation};
