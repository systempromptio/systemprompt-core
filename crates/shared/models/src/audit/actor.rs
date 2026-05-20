use std::fmt;

use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorKind {
    User,
    Job { job_name: String },
    Mcp { server_name: String },
}

impl ActorKind {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Job { .. } => "job",
            Self::Mcp { .. } => "mcp",
        }
    }

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
