//! [`AgentDispatch`]: sending one A2A request to an agent through the API's
//! agent proxy, for member code that must not hold the proxy.
//!
//! The proxy reads the secret store and the runtime configuration, so member
//! code never builds one. The API kernel binds an implementation into the
//! kernel-seeded, **sealed** [`AgentDispatchCell`]; only kernel code and the
//! provide fns and slot builds of core members read it (a sealed seed is
//! withheld from every member command, job and route state). A value a member
//! builds over it in its slot build sends only the requests that member
//! composes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::AgentName;

use crate::RequestContext;
use crate::late_bound::LateBound;

#[derive(Debug, thiserror::Error)]
pub enum AgentDispatchError {
    #[error("the request carries no bearer to present to the agent")]
    MissingBearer,
    #[error("the agent request could not be built")]
    Request(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("the agent proxy refused or failed the request")]
    Proxy(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("the agent response body could not be read")]
    ResponseBody(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Sends one A2A JSON-RPC body to an agent as the caller `context` names,
/// presenting the context's bearer, and returns the response body.
///
/// Injected as `dyn AgentDispatch`, hence `#[async_trait]`.
#[async_trait]
pub trait AgentDispatch: Send + Sync {
    async fn send(
        &self,
        agent: &AgentName,
        body: Vec<u8>,
        context: RequestContext,
    ) -> Result<Vec<u8>, AgentDispatchError>;
}

/// The kernel-seeded, sealed cell the API binds its agent dispatch into.
pub type AgentDispatchCell = LateBound<dyn AgentDispatch>;
