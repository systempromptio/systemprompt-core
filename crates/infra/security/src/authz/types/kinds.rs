//! Rule and entity kind tags with parse/display.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::authz::error::AuthzError;

/// Open subject-dimension vocabulary bound to `access_control_rules.rule_type`.
///
/// Core mints exactly two: [`RuleType::USER`] and [`RuleType::ROLE`]. Every
/// other dimension (department, cost centre, clearance, ...) is an extension
/// concern, minted with [`RuleType::extension`] and taught to the resolver via
/// a [`SubjectDimension`][sd] registered by
/// [`register_subject_attribute_provider!`][macro]. Core never interprets an
/// extension slug.
///
/// This mirrors [`AuthzContext`][ctx]: the column is an open vocabulary
/// validated at the Rust boundary rather than by a SQL `CHECK`, so an
/// unrecognised-but-well-formed slug is data, not an error.
///
/// [sd]: crate::authz::subject::SubjectDimension
/// [macro]: crate::register_subject_attribute_provider
/// [ctx]: super::request::AuthzContext
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuleType(Cow<'static, str>);

impl RuleType {
    /// Rule targeting one user by id.
    pub const USER: Self = Self(Cow::Borrowed("user"));
    /// Rule targeting every holder of a role.
    pub const ROLE: Self = Self(Cow::Borrowed("role"));

    /// Mints an extension-owned dimension slug.
    ///
    /// # Errors
    ///
    /// Returns [`AuthzError::InvalidRuleType`] when the slug is empty, is not
    /// lowercase `snake_case`, or collides with a core built-in. The shape
    /// requirement keeps dimensions from independent extensions from
    /// colliding with each other or with `user` / `role`.
    pub fn extension(slug: impl Into<Cow<'static, str>>) -> Result<Self, AuthzError> {
        let slug = slug.into();
        let well_formed = !slug.is_empty()
            && !slug.starts_with('_')
            && !slug.ends_with('_')
            && slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !well_formed || slug == Self::USER.as_str() || slug == Self::ROLE.as_str() {
            return Err(AuthzError::InvalidRuleType(slug.into_owned()));
        }
        Ok(Self(slug))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for RuleType {
    // Why: A rule type read back from storage is data, not input: an extension
    // dimension core has never heard of round-trips instead of poisoning the
    // read. Minting a *new* slug goes through [`RuleType::extension`], which
    // is where the shape rules are enforced.
    fn from(s: &str) -> Self {
        match s {
            "user" => Self::USER,
            "role" => Self::ROLE,
            other => Self(Cow::Owned(other.to_owned())),
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for RuleType {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        <str as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &<sqlx::Postgres as sqlx::Database>::TypeInfo) -> bool {
        <str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for RuleType {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode(self.as_str(), buf)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for RuleType {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let raw = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        Ok(Self::from(raw))
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
    SlackWorkspace,
    SlackChannel,
    TeamsTenant,
    TeamsConversation,
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
            Self::SlackWorkspace => "slack_workspace",
            Self::SlackChannel => "slack_channel",
            Self::TeamsTenant => "teams_tenant",
            Self::TeamsConversation => "teams_conversation",
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
