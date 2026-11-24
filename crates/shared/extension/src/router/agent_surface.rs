//! The agent HTTP surface contract.
//!
//! The `agent-http` member contributes an [`AgentSurface`]: one builder the
//! API mount calls with the route-scoped state of `agent-http`
//! ([`AgentRouteState`]). The member owns the handlers; the API owns the
//! layering around what the builder returns — each router's authorization
//! policy, request-context kind, rate scope and budget, and the route group it
//! is served in. The contract lives here, not in the agent crates, so the API
//! mounts the surface without naming the agent capability.
//!
//! [`AgentRouteState::resolved`] is the kernel resolution scoped to
//! `agent-http`: the open state and `agent-http`'s own sealed provides, and
//! nothing else sealed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;

use crate::{MissingDependency, Resolved};

/// Builds the webhook, stream and agent-card routers from the `agent-http`
/// route state.
pub trait AgentSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: AgentRouteState<'_>) -> Result<AgentSurfaceRoutes, AgentSurfaceError>;
}

pub type DynAgentSurface = Arc<dyn AgentSurface>;

/// The state the API mount hands an [`AgentSurface`].
#[derive(Debug)]
pub struct AgentRouteState<'a> {
    pub resolved: Resolved<'a>,
}

/// What an [`AgentSurface`] builds, each router still to be protected and
/// mounted by the API.
#[derive(Debug)]
pub struct AgentSurfaceRoutes {
    pub webhook: Router,
    pub stream: Router,
    pub agent_cards: Router,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum AgentSurfaceError {
    #[error(transparent)]
    Resolve(#[from] MissingDependency),
}
