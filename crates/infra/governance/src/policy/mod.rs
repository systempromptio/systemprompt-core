//! Unified tool-use governance plane.
//!
//! A governed call is an MCP tool invocation or a submitted prompt
//! ([`GovernedTarget`]), carrying tool arguments or prompt text
//! ([`GovernedInput`]). This module owns the whole enforcement runtime:
//!
//! - [`types`] — [`PolicyRequest`], an uncharged call; the policy contract
//!   ([`GovernancePolicy`], [`PolicyContext`]) lives in provider-contracts.
//! - [`config`] — the `governance.policies` YAML declaration
//!   ([`GovernanceConfig`]).
//! - [`registry`] — the [`PolicyCatalog`]: the [`builtin`] policies (secret
//!   scan, scope check, blocklist, rate limit, require approval) plus every
//!   admitted member's contributed policy.
//! - [`engine`] — [`GovernanceEngine`], the traced first-halt-wins evaluator.
//! - [`rate_window`] — the Postgres counters charged before evaluation.
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
pub mod plane;
pub mod rate_window;
pub mod registry;
pub mod secrets;
pub mod types;

pub use approval::{
    ApprovalOutcome, ApprovalRepository, ApprovalRequest, ApprovalStatus, ApprovalVerdict,
    NewApprovalRequest, args_digest, wait_for_decision,
};
pub use audit::{
    ApproverStamp, AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit,
    PrincipalSnapshot, record_decision, record_decision_with,
};
pub use builtin::{ApprovalSettings, SECRET_SCAN_ID};
pub use config::{GovernanceConfig, GovernanceConfigError, PolicyConfig};
pub use engine::{ChargedCall, Evaluation, GovernanceEngine, GovernanceEngineError};
pub use plane::{GOVERNANCE_FILE, GOVERNANCE_PLANE, GovernancePlane};
pub use rate_window::{RateCharge, RateWindowRepository};
pub use registry::{
    BuiltPolicy, BuiltinPolicy, PolicyCatalog, PolicyCatalogError, PolicyConfigurationError,
    PolicyFactory,
};
pub use secrets::{
    EntropyConfig, SecretPattern, SecretPatternError, SecretScanner, SignatureExemptions,
};
pub use systemprompt_provider_contracts::governance::{
    AccessScope, AgentScope, GovernancePolicy, GovernedInput, GovernedScalar, GovernedString,
    GovernedTarget, McpToolInput, PROMPT_TARGET_NAME, PolicyContext, PromptPart, RateCounts,
    RateLimitWindow, SecretFinding, SecretLocation, SecretSource, UNKNOWN_TARGET_NAME,
};
pub use types::PolicyRequest;
