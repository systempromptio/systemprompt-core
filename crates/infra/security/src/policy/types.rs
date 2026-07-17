//! Shared types for the unified governance plane.
//!
//! These types support the tool-use governance chain
//! ([`super::GovernancePolicy`]) and feed into the typed deny variants in
//! [`crate::authz::types::DenyReason`]. They live here (and not in
//! `authz/types.rs`) because they describe the *tool-call* enforcement plane
//! — secret scans, scope checks, blocklists, rate limits — which is
//! orthogonal to the user→entity allow/deny resolver.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpToolName, PolicyId, SessionId, UserId};

use crate::authz::error::AuthzError;
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

/// Permission tier carried alongside [`AgentScope`] in [`PolicyContext`].
///
/// `AgentScope` answers "who is acting" (user vs system process identity);
/// `AccessScope` answers "what permission tier is granted to this invocation"
/// (admin, plain user, unknown). The two are orthogonal — a system actor may
/// have any tier, a user actor may be admin or plain — so they live as
/// separate fields rather than a cartesian enum. `Unknown` is the fallback when
/// an agent card declares no `oauth.scopes` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AccessScope {
    Admin,
    User,
    Unknown,
}

impl AccessScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for AccessScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AccessScope {
    type Err = AuthzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(AuthzError::Validation(format!(
                "unknown access scope: {other}"
            ))),
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
pub struct McpToolInput(
    // JSON: MCP-protocol boundary — schema-less tool arguments mandated by the
    // spec. Governance is the only consumer that reaches into this blob.
    serde_json::Value,
);

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

#[derive(Debug)]
pub struct PolicyContext<'a> {
    pub tool: McpToolName,
    pub agent_scope: AgentScope,
    pub access_scope: AccessScope,
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub tool_input: &'a McpToolInput,
}

/// A unit of governance evaluation for an MCP tool call.
///
/// Implementations are pure-sync and side-effect free; auditing happens
/// outside the chain. First-deny-wins composition is provided by
/// [`super::GovernanceChain`].
pub trait GovernancePolicy: Send + Sync + fmt::Debug {
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
