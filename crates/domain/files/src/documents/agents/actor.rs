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
#[cfg(any(test, feature = "test-harness"))]
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

tokio::task_local! {
    static ACTOR: DocumentActor;
}

/// An agent acting for the signed-in user. `attended` is `true` while a
/// person is in the session that drives it, `false` for a headless run.
/// `channel` is the path the call came in on, set by core's own routes, so a
/// policy can allow a capability to one path (the Drive sync's whole-file
/// saves) and not to the client that drives it generally.
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
    /// Tools, REST and anything else the agent drives itself.
    #[default]
    Direct,
    /// The bridge Drive sync's manifest, feed, blob and commit routes.
    DriveSync,
    /// A live co-editing session's checkpoint of its edits.
    LiveCheckpoint,
}

impl AgentActor {
    /// The same agent on `channel`.
    #[must_use]
    pub fn on(mut self, channel: AgentChannel) -> Self {
        self.channel = channel;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentActor {
    Human,
    Agent(AgentActor),
    /// Core's own background work (a scheduled job, an ingest), named for
    /// its audit trail. Trusted like a person: never capped, never recorded
    /// as an agent that created a document.
    System {
        name: String,
    },
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

    /// This actor on `channel`; a person or the system is unchanged.
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

/// Runs `work` with `actor` as the actor of every document service call it
/// makes on this task.
pub async fn act_as<F: Future>(actor: DocumentActor, work: F) -> F::Output {
    ACTOR.scope(actor, work).await
}

/// The name an agent goes by when nothing signed names it.
pub const UNNAMED_AGENT: &str = "agent";

#[cfg(any(test, feature = "test-harness"))]
static UNSCOPED: OnceLock<DocumentActor> = OnceLock::new();

/// The actor of a call outside any [`act_as`] scope: an unnamed,
/// unattended agent (a test harness may replace it with
/// `set_unscoped_actor`, behind the `test-harness` feature).
#[must_use]
pub fn unscoped_actor() -> DocumentActor {
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(actor) = UNSCOPED.get() {
        return actor.clone();
    }
    DocumentActor::agent(UNNAMED_AGENT, false)
}

/// Replaces the unscoped default, once per process. Only for a test harness
/// that drives the document service directly as a person: it exists only
/// with the `test-harness` feature, which a server build never enables.
/// Returns `false` when a default was already set.
#[cfg(any(test, feature = "test-harness"))]
pub fn set_unscoped_actor(actor: DocumentActor) -> bool {
    UNSCOPED.set(actor).is_ok()
}

/// The actor of the running call: the innermost [`act_as`] scope, or
/// [`unscoped_actor`] outside one.
#[must_use]
pub fn current_actor() -> DocumentActor {
    ACTOR
        .try_with(Clone::clone)
        .unwrap_or_else(|_| unscoped_actor())
}
