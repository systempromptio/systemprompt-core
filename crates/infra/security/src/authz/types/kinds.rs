use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::authz::error::AuthzError;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
