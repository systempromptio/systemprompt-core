//! Declared toolsets and per-tool access for one MCP server.
//!
//! A [`ToolsetRef`] names a toolset compiled into the binary (registered
//! through the runtime's `submit_toolset!`) that a deployment hosts. A
//! [`ToolAccess`] narrows who may see and call a tool: it is satisfied when the
//! caller holds any listed scope, any listed role, or any listed group.
//!
//! Access resolves most specific first: a tool's own `access`, then the
//! `access` of the toolset that serves it, then the server's `oauth.scopes`
//! (already enforced for every request by RBAC). [`ResolvedAccess`] is that
//! decision, and [`ResolvedAccess::permits`] judges one caller against it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::auth::{AuthenticatedUser, Permission};

/// A toolset a deployment hosts, by the id its factory registers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolsetRef {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<ToolAccess>,
}

const fn default_enabled() -> bool {
    true
}

/// Who may see and call a tool; any one match grants access.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolAccess {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<Permission>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

impl ToolAccess {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty() && self.roles.is_empty() && self.groups.is_empty()
    }

    #[must_use]
    pub fn needs_groups(&self) -> bool {
        !self.groups.is_empty()
    }

    #[must_use]
    pub fn permits(&self, user: Option<&AuthenticatedUser>, groups: &[String]) -> bool {
        let Some(user) = user else {
            return false;
        };
        let scope_match = self
            .scopes
            .iter()
            .any(|required| user.permissions.iter().any(|held| held.implies(required)));
        let role_match = self.roles.iter().any(|role| user.roles.contains(role));
        let group_match = self.groups.iter().any(|group| groups.contains(group));
        scope_match || role_match || group_match
    }
}

/// Where a tool's effective access came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedAccess<'a> {
    Tool(&'a ToolAccess),
    Toolset(&'a ToolAccess),
    Server,
}

impl<'a> ResolvedAccess<'a> {
    #[must_use]
    pub fn resolve(tool: Option<&'a ToolAccess>, toolset: Option<&'a ToolAccess>) -> Self {
        match (tool, toolset) {
            (Some(access), _) => Self::Tool(access),
            (None, Some(access)) => Self::Toolset(access),
            (None, None) => Self::Server,
        }
    }

    #[must_use]
    pub const fn access(&self) -> Option<&'a ToolAccess> {
        match *self {
            Self::Tool(access) | Self::Toolset(access) => Some(access),
            Self::Server => None,
        }
    }

    #[must_use]
    pub fn needs_groups(&self) -> bool {
        self.access().is_some_and(ToolAccess::needs_groups)
    }

    #[must_use]
    pub fn permits(&self, user: Option<&AuthenticatedUser>, groups: &[String]) -> bool {
        self.access()
            .is_none_or(|access| access.permits(user, groups))
    }
}
