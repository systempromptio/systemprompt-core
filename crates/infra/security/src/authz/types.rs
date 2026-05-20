//! Wire and storage types for authorization decisions.
//!
//! Types fall into two groups:
//!
//! 1. **Storage** — [`RuleType`], [`Access`], [`AccessRule`] map to columns in
//!    `access_control_rules`. They round-trip through serde and sqlx.
//! 2. **Decision** — [`Decision`] is the in-process resolver output;
//!    [`AuthzRequest`] / [`AuthzDecision`] are the webhook wire format sent to
//!    and parsed back from extension hook handlers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{Actor, RuleId, TraceId, UserId};

use super::error::AuthzError;

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
    pub default_included: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Deny {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthzRequest {
    pub entity_type: EntityKind,
    pub entity_id: String,
    pub user_id: UserId,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub department: String,
    pub trace_id: TraceId,
    #[serde(default)]
    // JSON: extension hook contract — context is forwarded verbatim to webhook
    // handlers and is intentionally schema-free at this boundary.
    pub context: serde_json::Value,
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
    Deny { reason: String, policy: String },
}
