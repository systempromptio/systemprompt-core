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
use std::sync::OnceLock;

use serde::Serialize;

tokio::task_local! {
    static ACTOR: DocumentActor;
}

/// An agent acting for the signed-in user. `attended` is `true` while a
/// person is in the session that drives it, `false` for a headless run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentActor {
    pub name: String,
    pub attended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentActor {
    Human,
    Agent(AgentActor),
}

impl DocumentActor {
    #[must_use]
    pub fn agent(name: impl Into<String>, attended: bool) -> Self {
        Self::Agent(AgentActor {
            name: name.into(),
            attended,
        })
    }

    #[must_use]
    pub const fn as_agent(&self) -> Option<&AgentActor> {
        match self {
            Self::Human => None,
            Self::Agent(agent) => Some(agent),
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

static UNSCOPED: OnceLock<DocumentActor> = OnceLock::new();

/// The actor of a call outside any [`act_as`] scope: an unnamed,
/// unattended agent, unless [`set_unscoped_actor`] said otherwise.
#[must_use]
pub fn unscoped_actor() -> DocumentActor {
    UNSCOPED
        .get()
        .cloned()
        .unwrap_or_else(|| DocumentActor::agent(UNNAMED_AGENT, false))
}

/// Replaces the unscoped default, once per process. Only for a test harness
/// that drives the document service directly as a person; a server never
/// calls it, so its unscoped calls stay capped. Returns `false` when a
/// default was already set.
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
