//! [`DocumentActor`]: who a document service call runs as.
//!
//! The actor rides a task-local scope rather than every method's signature:
//! the surface that knows it (the MCP tools derive it from the request
//! context) wraps the call in [`act_as`], and the access service reads it
//! with [`current_actor`] at the one place every check passes through. A
//! call outside any scope (the HTTP routes, the live editor, jobs) acts as a
//! person, which is uncapped, so an agent surface that forgets the scope
//! fails open to the user's own level and no further.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

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

/// The actor of the running call: the innermost [`act_as`] scope, or
/// [`DocumentActor::Human`] outside one.
#[must_use]
pub fn current_actor() -> DocumentActor {
    ACTOR.try_with(Clone::clone).unwrap_or(DocumentActor::Human)
}
