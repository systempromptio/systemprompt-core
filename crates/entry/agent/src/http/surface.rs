//! [`AgentHttpSurface`]: the webhook, stream and agent-card routers the API
//! mounts for the `agent-http` member with its own layering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_agent::services::ContextProviderService;
use systemprompt_agent::services::registry::AgentCardSite;
use systemprompt_extension::FromState;
use systemprompt_extension::router::{
    AgentRouteState, AgentSurface, AgentSurfaceError, AgentSurfaceRoutes,
};
use systemprompt_manifest::services::ServicesView;

use super::agent_cards::{self, AgentCardsState};
use super::stream::{self, StreamFeeds, StreamState};
use super::{AgentHttpState, webhook};

/// The `agent-http` member's [`AgentSurface`].
///
/// The webhook broadcasts run over [`AgentHttpState`], the streams over the
/// member's [`StreamFeeds`] and a context provider on the same repositories,
/// and the well-known agent cards over the services view and the open
/// [`AgentCardSite`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentHttpSurface;

impl AgentSurface for AgentHttpSurface {
    fn build(&self, state: AgentRouteState<'_>) -> Result<AgentSurfaceRoutes, AgentSurfaceError> {
        let r = &state.resolved;
        let http = AgentHttpState::from_state(r)?;
        let stream = StreamState {
            contexts: Arc::new(ContextProviderService::new(
                http.repositories.contexts.clone(),
            )),
            feeds: r.get::<Arc<StreamFeeds>>()?,
        };
        let cards = AgentCardsState {
            services: r.get::<ServicesView>()?,
            site: r.get::<Arc<AgentCardSite>>()?,
        };
        Ok(AgentSurfaceRoutes {
            webhook: webhook::router(http),
            stream: stream::router(stream),
            agent_cards: agent_cards::router(cards),
        })
    }
}
