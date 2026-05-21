//! Principal + surface attribution for audit and event rows.
//!
//! Every actor-bearing row persists `(user_id, kind, kind.actor_id())` as a
//! unit; the three values cannot be separated at the call site because they
//! live inside [`Actor`]. The `user_id` is always a real `users` row — the
//! kind disambiguates which surface ran on that user's behalf.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::UserId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub user_id: UserId,
    pub kind: ActorKind,
}

impl Actor {
    #[must_use]
    pub const fn user(user_id: UserId) -> Self {
        Self {
            user_id,
            kind: ActorKind::User,
        }
    }

    /// Unauthenticated traffic that has already been bound to a real
    /// (typically ephemeral) `anonymous_*` user row. The `user_id` is the
    /// provisioned row's id, not a sentinel.
    #[must_use]
    pub const fn anonymous(user_id: UserId) -> Self {
        Self {
            user_id,
            kind: ActorKind::Anonymous,
        }
    }

    /// Platform-originated work (bootstrap jobs, scheduler tick, internal
    /// fallbacks). The caller passes the resolved system-admin user id;
    /// no sentinel is fabricated inside the constructor.
    #[must_use]
    pub const fn system(user_id: UserId) -> Self {
        Self {
            user_id,
            kind: ActorKind::System,
        }
    }

    #[must_use]
    pub fn job(user_id: UserId, job_name: impl Into<String>) -> Self {
        Self {
            user_id,
            kind: ActorKind::Job {
                job_name: job_name.into(),
            },
        }
    }

    #[must_use]
    pub fn mcp(user_id: UserId, server_name: impl Into<String>) -> Self {
        Self {
            user_id,
            kind: ActorKind::Mcp {
                server_name: server_name.into(),
            },
        }
    }

    /// A configured agent (Claude Code session, autonomous agent, etc.)
    /// acting on the user's behalf. The agent is the surface; the user is
    /// the accountable principal.
    #[must_use]
    pub fn agent(user_id: UserId, agent_id: impl Into<String>) -> Self {
        Self {
            user_id,
            kind: ActorKind::Agent {
                agent_id: agent_id.into(),
            },
        }
    }

    #[must_use]
    pub fn audit_columns(&self) -> (&str, &str) {
        (self.kind.as_str(), self.kind.actor_id(&self.user_id))
    }

    #[must_use]
    pub fn from_tool_name(user_id: UserId, agent_id: Option<&str>, tool_name: &str) -> Self {
        if let Some(rest) = tool_name.strip_prefix("mcp__") {
            if let Some(server) = rest.split("__").next() {
                if !server.is_empty() {
                    return Self::mcp(user_id, server);
                }
            }
        }
        match agent_id {
            Some(id) if !id.is_empty() => Self::agent(user_id, id),
            _ => Self::user(user_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActorKind {
    User,
    Anonymous,
    System,
    Job { job_name: String },
    Mcp { server_name: String },
    Agent { agent_id: String },
}

impl ActorKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Anonymous => "anonymous",
            Self::System => "system",
            Self::Job { .. } => "job",
            Self::Mcp { .. } => "mcp",
            Self::Agent { .. } => "agent",
        }
    }

    #[must_use]
    pub fn actor_id<'a>(&'a self, user_id: &'a UserId) -> &'a str {
        match self {
            Self::User | Self::Anonymous | Self::System => user_id.as_str(),
            Self::Job { job_name } => job_name.as_str(),
            Self::Mcp { server_name } => server_name.as_str(),
            Self::Agent { agent_id } => agent_id.as_str(),
        }
    }
}

impl ActorKind {
    #[must_use]
    pub const fn tag(&self) -> ActorKindTag {
        match self {
            Self::User => ActorKindTag::User,
            Self::Anonymous => ActorKindTag::Anonymous,
            Self::System => ActorKindTag::System,
            Self::Job { .. } => ActorKindTag::Job,
            Self::Mcp { .. } => ActorKindTag::Mcp,
            Self::Agent { .. } => ActorKindTag::Agent,
        }
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Discriminant-only view of [`ActorKind`], bound to the `actor_kind` column
/// in `governance_decisions`.
///
/// Binding a typed value couples the SQL CHECK allow-list to the enum at
/// compile time; adding a variant without extending the constraint fails the
/// build instead of silently rejecting rows at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "snake_case"))]
#[serde(rename_all = "snake_case")]
pub enum ActorKindTag {
    User,
    Anonymous,
    System,
    Job,
    Mcp,
    Agent,
}

impl ActorKindTag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Anonymous => "anonymous",
            Self::System => "system",
            Self::Job => "job",
            Self::Mcp => "mcp",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for ActorKindTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
