//! Rule and entity kind tags with parse/display.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::authz::error::AuthzError;

pub use systemprompt_models::authz::RuleType;

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
    SlackWorkspace,
    SlackChannel,
    TeamsTenant,
    TeamsConversation,
    Model,
    Provider,
}

impl EntityKind {
    pub const ALL: &'static [Self] = &[
        Self::GatewayRoute,
        Self::McpServer,
        Self::Plugin,
        Self::Agent,
        Self::Marketplace,
        Self::Skill,
        Self::Hook,
        Self::SlackWorkspace,
        Self::SlackChannel,
        Self::TeamsTenant,
        Self::TeamsConversation,
        Self::Model,
        Self::Provider,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GatewayRoute => "gateway_route",
            Self::McpServer => "mcp_server",
            Self::Plugin => "plugin",
            Self::Agent => "agent",
            Self::Marketplace => "marketplace",
            Self::Skill => "skill",
            Self::Hook => "hook",
            Self::SlackWorkspace => "slack_workspace",
            Self::SlackChannel => "slack_channel",
            Self::TeamsTenant => "teams_tenant",
            Self::TeamsConversation => "teams_conversation",
            Self::Model => "model",
            Self::Provider => "provider",
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
            "slack_workspace" => Ok(Self::SlackWorkspace),
            "slack_channel" => Ok(Self::SlackChannel),
            "teams_tenant" => Ok(Self::TeamsTenant),
            "teams_conversation" => Ok(Self::TeamsConversation),
            "model" => Ok(Self::Model),
            "provider" => Ok(Self::Provider),
            other => Err(AuthzError::UnknownEntityKind(other.to_owned())),
        }
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl sqlx::Type<sqlx::Postgres> for EntityKind {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &<sqlx::Postgres as sqlx::Database>::TypeInfo) -> bool {
        <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for EntityKind {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode(self.as_str(), buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for EntityKind {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let raw = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(Self::from_str(raw)?)
    }
}
