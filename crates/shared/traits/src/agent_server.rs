//! [`AgentServer`]: the seam through which a spawned agent process serves one
//! agent without the kernel naming the agent domain.
//!
//! The CLI kernel's child-host command boots the host context the API granted
//! the process and reads a [`DynAgentServer`] from its state. The agent member
//! provides it on that host, built in its provide fn from the host's own
//! seeds, so the command never holds the collaborators an agent serves with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::AgentName;

use crate::BoxedSource;

#[derive(Debug, thiserror::Error)]
pub enum AgentServeError {
    #[error("agent {agent} could not be served")]
    Serve {
        agent: AgentName,
        #[source]
        source: BoxedSource,
    },
}

/// Serves one agent on a port until the process is signalled, then drains
/// whatever the agent's collaborators still owe (request audit writes).
///
/// Injected as `dyn AgentServer`, hence `#[async_trait]`.
#[async_trait]
pub trait AgentServer: Send + Sync {
    async fn serve(&self, agent: &AgentName, port: u16) -> Result<(), AgentServeError>;
}

pub type DynAgentServer = Arc<dyn AgentServer>;
