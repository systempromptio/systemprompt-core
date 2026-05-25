//! Shared types for the unified governance plane.
//!
//! These types support the tool-use governance chain
//! ([`super::GovernancePolicy`]) and feed into the typed deny variants in
//! [`crate::authz::types::DenyReason`]. They live here (and not in
//! `authz/types.rs`) because they describe the *tool-call* enforcement plane
//! — secret scans, scope checks, blocklists, rate limits — which is
//! orthogonal to the user→entity allow/deny resolver.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpToolName, PolicyId, SessionId, UserId};

use crate::authz::types::Decision;

/// Where in a tool-call payload a secret-scanner finding was located.
///
/// `kind` identifies the field family (e.g. `"arg"`, `"env"`); `path` is the
/// JSON-pointer-like dotted path within the tool input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretLocation {
    pub kind: String,
    pub path: String,
}

impl SecretLocation {
    pub fn new(kind: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            path: path.into(),
        }
    }
}

/// Configured rate-limit window the caller exceeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitWindow {
    pub name: String,
    pub seconds: u64,
    pub limit: u64,
}

/// Scope of an agent invocation for governance evaluation. Agents may run
/// either inside an authenticated user session or under a system/service
/// identity (cron, replay, internal scheduler).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentScope {
    User { user_id: UserId },
    System,
}

impl AgentScope {
    #[must_use]
    pub const fn user_id(&self) -> Option<&UserId> {
        match self {
            Self::User { user_id } => Some(user_id),
            Self::System => None,
        }
    }
}

/// Untyped MCP tool input wrapped at the protocol boundary.
///
/// The MCP protocol mandates schema-less JSON for tool arguments — every tool
/// defines its own input shape. This wrapper is the single point where
/// governance reaches into that JSON; everywhere else the typed path is
/// preferred. Callers extract fields via [`Self::as_str`] / [`Self::as_path`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpToolInput(serde_json::Value);

impl McpToolInput {
    #[must_use]
    pub const fn new(value: serde_json::Value) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self, field: &str) -> Option<&str> {
        self.0.get(field).and_then(serde_json::Value::as_str)
    }

    #[must_use]
    pub fn as_path(&self, field: &str) -> Option<&str> {
        self.as_str(field)
    }
}

/// Per-evaluation context handed to every policy in a [`super::GovernanceChain`].
#[derive(Debug)]
pub struct PolicyContext<'a> {
    pub tool: McpToolName,
    pub agent_scope: AgentScope,
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub tool_input: &'a McpToolInput,
}

/// A unit of governance evaluation for an MCP tool call.
///
/// Implementations are pure-sync and side-effect free; auditing happens
/// outside the chain. First-deny-wins composition is provided by
/// [`super::GovernanceChain`].
pub trait GovernancePolicy: Send + Sync + std::fmt::Debug {
    fn id(&self) -> PolicyId;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision;
}

/// Ordered chain of [`GovernancePolicy`] evaluated first-deny-wins.
#[derive(Debug, Clone, Default)]
pub struct GovernanceChain {
    entries: Vec<Arc<dyn GovernancePolicy>>,
}

impl GovernanceChain {
    #[must_use]
    pub const fn new(entries: Vec<Arc<dyn GovernancePolicy>>) -> Self {
        Self { entries }
    }

    pub fn push(&mut self, policy: Arc<dyn GovernancePolicy>) {
        self.entries.push(policy);
    }

    #[must_use]
    pub fn entries(&self) -> &[Arc<dyn GovernancePolicy>] {
        &self.entries
    }

    /// Evaluate every policy in order. The first [`Decision::Deny`] short-circuits;
    /// if all policies allow, fall through to
    /// [`crate::authz::types::MatchedBy::DefaultIncluded`].
    #[must_use]
    pub fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        for policy in &self.entries {
            if let deny @ Decision::Deny { .. } = policy.evaluate(ctx) {
                return deny;
            }
        }
        Decision::Allow {
            matched_by: crate::authz::types::MatchedBy::DefaultIncluded,
        }
    }
}
