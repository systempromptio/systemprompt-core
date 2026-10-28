//! [`DocumentActor`]: who a document service call runs as.
//!
//! The actor rides a task-local scope rather than every method's signature:
//! each surface derives it from the request's credential (the MCP tools and
//! the HTTP routes alike) and wraps the call in [`act_as`], and the access
//! service reads it with [`current_actor`] at the one place every check
//! passes through.
//!
//! The default fails closed: a call outside any scope acts as an unnamed,
//! unattended agent ([`unscoped_actor`]), so a surface that forgets the
//! scope is capped rather than trusted. Only an explicit
//! [`DocumentActor::Human`] scope is uncapped, and a surface sets one only
//! for a first-party browser session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use serde::{Deserialize, Serialize};

tokio::task_local! {
    static ACTOR: DocumentActor;
}

/// An agent acting for the signed-in user.
///
/// `attended` is `true` while a person is in the session that drives it,
/// `false` for a headless run. `channel` is the path the call came in on, set
/// by core's own routes, so a policy can allow a capability to one path (the
/// Drive sync's whole-file saves) and not to the client that drives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentActor {
    pub name: String,
    pub attended: bool,
    pub channel: AgentChannel,
}

/// The path an agent's call came in on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentChannel {
    #[default]
    Direct,
    DriveSync,
    LiveCheckpoint,
}

impl AgentActor {
    #[must_use]
    pub const fn on(mut self, channel: AgentChannel) -> Self {
        self.channel = channel;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentActor {
    Human,
    Agent(AgentActor),
    System { name: String },
}

impl DocumentActor {
    #[must_use]
    pub fn agent(name: impl Into<String>, attended: bool) -> Self {
        Self::Agent(AgentActor {
            name: name.into(),
            attended,
            channel: AgentChannel::Direct,
        })
    }

    #[must_use]
    pub fn system(name: impl Into<String>) -> Self {
        Self::System { name: name.into() }
    }

    #[must_use]
    pub const fn as_agent(&self) -> Option<&AgentActor> {
        match self {
            Self::Human | Self::System { .. } => None,
            Self::Agent(agent) => Some(agent),
        }
    }

    #[must_use]
    pub fn on(self, channel: AgentChannel) -> Self {
        match self {
            Self::Agent(agent) => Self::Agent(agent.on(channel)),
            other => other,
        }
    }

    #[must_use]
    pub const fn is_agent(&self) -> bool {
        matches!(self, Self::Agent(_))
    }
}

pub async fn act_as<F: Future>(actor: DocumentActor, work: F) -> F::Output {
    ACTOR.scope(actor, work).await
}

pub const UNNAMED_AGENT: &str = "agent";

#[must_use]
pub fn unscoped_actor() -> DocumentActor {
    DocumentActor::agent(UNNAMED_AGENT, false)
}

#[must_use]
pub fn scoped_actor() -> Option<DocumentActor> {
    ACTOR.try_with(Clone::clone).ok()
}

#[must_use]
pub fn current_actor() -> DocumentActor {
    scoped_actor().unwrap_or_else(unscoped_actor)
}
