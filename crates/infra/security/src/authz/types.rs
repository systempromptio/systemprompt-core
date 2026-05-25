//! Wire and storage types for authorization decisions.
//!
//! Types fall into two groups:
//!
//! 1. **Storage** — [`RuleType`], [`Access`], [`AccessRule`] map to columns in
//!    `access_control_rules`. They round-trip through serde and sqlx.
//! 2. **Decision** — [`Decision`] is the in-process resolver output;
//!    [`AuthzRequest`] / [`AuthzDecision`] are the webhook wire format sent to
//!    and parsed back from extension hook handlers.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    Actor, AgentId, HookId, MarketplaceId, McpServerId, McpToolName, ModelId, PluginId, PolicyId,
    RouteId, RuleId, SecretPatternId, SkillId, TraceId, UserId,
};
use thiserror::Error;

use super::error::AuthzError;
use crate::policy::types::{RateLimitWindow, SecretLocation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RuleType {
    User,
    Role,
    Department,
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::User => "user",
            Self::Role => "role",
            Self::Department => "department",
        })
    }
}

impl FromStr for RuleType {
    type Err = AuthzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "role" => Ok(Self::Role),
            "department" => Ok(Self::Department),
            other => Err(AuthzError::InvalidRuleType(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Allow,
    Deny,
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        })
    }
}

impl FromStr for Access {
    type Err = AuthzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            other => Err(AuthzError::InvalidAccess(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    GatewayRoute,
    McpServer,
    Plugin,
    Agent,
    Marketplace,
    Skill,
    Hook,
}

impl EntityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GatewayRoute => "gateway_route",
            Self::McpServer => "mcp_server",
            Self::Plugin => "plugin",
            Self::Agent => "agent",
            Self::Marketplace => "marketplace",
            Self::Skill => "skill",
            Self::Hook => "hook",
        }
    }
}

impl FromStr for EntityKind {
    type Err = AuthzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gateway_route" => Ok(Self::GatewayRoute),
            "mcp_server" => Ok(Self::McpServer),
            "plugin" => Ok(Self::Plugin),
            "agent" => Ok(Self::Agent),
            "marketplace" => Ok(Self::Marketplace),
            "skill" => Ok(Self::Skill),
            "hook" => Ok(Self::Hook),
            other => Err(AuthzError::Validation(format!(
                "unknown entity_type: {other}"
            ))),
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccessRule {
    pub id: RuleId,
    pub rule_type: RuleType,
    pub rule_value: String,
    pub access: Access,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// One row from `access_control_entities`.
///
/// Owns the per-entity `default_included` flag and a provenance string
/// identifying which loader pass first registered the entity
/// (`"profile:<name>"`, `"roles.yaml"`, `"departments.yaml"`, or `"legacy:*"`
/// for pre-split rows). Callers pair this with [`AccessRule`]s from
/// `access_control_rules` and hand both to [`super::resolver::resolve`].
///
/// A `None` lookup result means the entity is unknown to access control and
/// the resolver returns [`DenyReason::UnknownEntity`] rather than the generic
/// `NotAssigned`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRow {
    pub kind: EntityKind,
    pub id: String,
    pub default_included: bool,
    pub source: String,
}

/// Why an [`AuthzRequest`] was allowed. Carries enough structure for the
/// audit row to attribute the decision without re-deriving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchedBy {
    UserAllow,
    RoleAllow {
        role: String,
    },
    DepartmentAllow {
        department: String,
    },
    /// No matching rule, but the entity's `default_included` flag was set.
    DefaultIncluded,
    /// Allowed by a named tool-use governance policy (secret scan, etc).
    PolicyAllow {
        policy_id: PolicyId,
        detail: Cow<'static, str>,
    },
}

/// Structured deny rationale.
///
/// Variants cover both the user→entity resolver
/// (`UserDeny`, `RoleDeny`, `DepartmentDeny`, `NotAssigned`, `UnknownEntity`),
/// the hook plane (`HookUnavailable`), and the tool-use governance chain
/// (`SecretLeak`, `ScopeViolation`, `ToolBlocked`, `RateLimitExceeded`). The
/// human-readable `#[error]` strings double as the `reason` column in the
/// `governance_decisions` audit row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DenyReason {
    #[error("user {user_id} explicitly denied for {entity}")]
    UserDeny {
        entity: EntityRef,
        user_id: UserId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error("role {role} denied for {entity}")]
    RoleDeny {
        entity: EntityRef,
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error("department {department} denied for {entity}")]
    DepartmentDeny {
        entity: EntityRef,
        department: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error(
        "{entity}: not assigned to user {user_id} with roles {roles:?} (no allow rule; \
         default_included = false). Add an allow rule in services/access-control/roles.yaml."
    )]
    NotAssigned {
        entity: EntityRef,
        user_id: UserId,
        roles: Vec<String>,
    },
    #[error(
        "{entity}: unknown to access control. Add an entity row via the publish pipeline or \
         roles.yaml."
    )]
    UnknownEntity { entity: EntityRef },
    #[error("authz hook unavailable for policy {policy}")]
    HookUnavailable { policy: String },
    #[error("secret pattern {pattern_id} detected at {location:?}")]
    SecretLeak {
        pattern_id: SecretPatternId,
        location: SecretLocation,
    },
    #[error("tool {tool} missing required scope {missing_scope}")]
    ScopeViolation {
        tool: McpToolName,
        missing_scope: String,
    },
    #[error("tool {tool} blocked by list {list_id}")]
    ToolBlocked {
        tool: McpToolName,
        list_id: String,
    },
    #[error("rate limit {window:?} exceeded; retry after {retry_after_ms}ms")]
    RateLimitExceeded {
        window: RateLimitWindow,
        retry_after_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum Decision {
    Allow { matched_by: MatchedBy },
    Deny { reason: DenyReason },
}

impl Decision {
    #[must_use]
    pub const fn tag(&self) -> DecisionTag {
        match self {
            Self::Allow { .. } => DecisionTag::Allow,
            Self::Deny { .. } => DecisionTag::Deny,
        }
    }
}

/// Discriminant-only view of [`Decision`] / [`AuthzDecision`], bound to the
/// `governance_decisions.decision` column.
///
/// Typing the column at the Rust boundary couples it to the SQL CHECK
/// allow-list; adding a `Decision` variant without extending the constraint
/// fails the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DecisionTag {
    Allow,
    Deny,
}

impl DecisionTag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

impl fmt::Display for DecisionTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&AuthzDecision> for DecisionTag {
    fn from(d: &AuthzDecision) -> Self {
        match d {
            AuthzDecision::Allow => Self::Allow,
            AuthzDecision::Deny { .. } => Self::Deny,
        }
    }
}

/// Tagged-union reference to an authz target. Bundles the discriminator
/// (`EntityKind`) and the typed id so they can never drift apart on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum EntityRef {
    GatewayRoute(RouteId),
    McpServer(McpServerId),
    Plugin(PluginId),
    Agent(AgentId),
    Marketplace(MarketplaceId),
    Skill(SkillId),
    Hook(HookId),
}

impl EntityRef {
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        match self {
            Self::GatewayRoute(_) => EntityKind::GatewayRoute,
            Self::McpServer(_) => EntityKind::McpServer,
            Self::Plugin(_) => EntityKind::Plugin,
            Self::Agent(_) => EntityKind::Agent,
            Self::Marketplace(_) => EntityKind::Marketplace,
            Self::Skill(_) => EntityKind::Skill,
            Self::Hook(_) => EntityKind::Hook,
        }
    }

    #[must_use]
    pub fn id_str(&self) -> &str {
        match self {
            Self::GatewayRoute(id) => id.as_str(),
            Self::McpServer(id) => id.as_str(),
            Self::Plugin(id) => id.as_str(),
            Self::Agent(id) => id.as_str(),
            Self::Marketplace(id) => id.as_str(),
            Self::Skill(id) => id.as_str(),
            Self::Hook(id) => id.as_str(),
        }
    }
}

impl fmt::Display for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind().as_str(), self.id_str())
    }
}

/// Typed per-request context forwarded to the authz hook. The variant is
/// the type of enforcement site that fired; downstream policy handlers
/// pattern-match instead of poking at `serde_json::Value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthzContext {
    /// Gateway `/v1/messages` invocation — carries the model literal the
    /// client requested (the route in `entity` already encodes routing
    /// policy, but the literal is needed for audit and downstream rules).
    GatewayInvocation { model: ModelId },
    /// MCP tool call about to be dispatched.
    McpToolCall { tool: McpToolName },
    /// No context (RBAC server-attach checks, etc).
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzRequest {
    pub entity: EntityRef,
    pub user_id: UserId,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub department: String,
    pub trace_id: TraceId,
    #[serde(default)]
    pub context: AuthzContext,
    /// RFC 8693 delegation lineage forwarded from
    /// `RequestContext.auth.act_chain`. Empty when no token-exchange chain
    /// is present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub act_chain: Vec<Actor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum AuthzDecision {
    Allow,
    Deny { reason: DenyReason, policy: String },
}
