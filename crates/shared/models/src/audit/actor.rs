use std::fmt;

use systemprompt_identifiers::UserId;

/// Principal + surface attribution for an action.
///
/// Every actor-bearing audit row persists `(user_id, kind, kind.actor_id())`
/// as a unit — the three values cannot be separated at the call site because
/// they live inside this struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user_id: UserId,
    pub kind: ActorKind,
}

impl Actor {
    /// A real user acting directly through an authenticated session. The user
    /// IS the actor; there is no intermediate surface.
    #[must_use]
    pub const fn user(user_id: UserId) -> Self {
        Self {
            user_id,
            kind: ActorKind::User,
        }
    }

    /// A scheduled job running on the configured owner's behalf.
    #[must_use]
    pub fn job(user_id: UserId, job_name: impl Into<String>) -> Self {
        Self {
            user_id,
            kind: ActorKind::Job {
                job_name: job_name.into(),
            },
        }
    }

    /// An MCP tool invocation dispatched by a configured server with no human
    /// session in the path.
    #[must_use]
    pub fn mcp(user_id: UserId, server_name: impl Into<String>) -> Self {
        Self {
            user_id,
            kind: ActorKind::Mcp {
                server_name: server_name.into(),
            },
        }
    }
}

/// Discriminator for the surface that performed an action.
///
/// The variant carries the surface-specific identifier (job name, MCP server
/// name) so writers cannot persist a mismatched `(kind, actor_id)` pair —
/// both columns are derived from the same enum value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorKind {
    User,
    Job { job_name: String },
    Mcp { server_name: String },
}

impl ActorKind {
    /// Stable string for the `actor_kind` column. Matches the
    /// `CHECK (actor_kind IN (...))` clause on every audit table.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Job { .. } => "job",
            Self::Mcp { .. } => "mcp",
        }
    }

    /// Surface-specific identifier for the `actor_id` column. For `User`
    /// actors the `actor_id` is the `user_id` itself — the user is the actor.
    #[must_use]
    pub fn actor_id<'a>(&'a self, user_id: &'a UserId) -> &'a str {
        match self {
            Self::User => user_id.as_str(),
            Self::Job { job_name } => job_name.as_str(),
            Self::Mcp { server_name } => server_name.as_str(),
        }
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
