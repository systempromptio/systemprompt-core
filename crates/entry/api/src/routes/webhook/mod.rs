//! Webhook broadcast surface for context events.
//!
//! Receives A2A and AG-UI webhook payloads, loads the referenced entities, and
//! fans the resulting events out to subscribed context streams. The kernel
//! mounts it at `/api/v1/webhook` behind the authenticated gate with a user
//! request context and no rate limit, a shape a member route set cannot
//! declare.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod broadcast_handlers;
mod context_broadcast;
pub mod error;
pub mod event_loader;
pub mod types;
pub mod validation;

use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_events::EventRouter;
use systemprompt_extension::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_traits::DynSessionUsageCounters;

pub use broadcast_handlers::{broadcast_a2a_event, broadcast_agui_event};
pub use context_broadcast::broadcast_context_event;
pub use types::{A2ABroadcastRequest, AgUiBroadcastRequest, WebhookRequest};

/// The repositories the webhook broadcasts load the referenced entities from.
#[derive(Clone)]
pub struct WebhookRepositories {
    pub contexts: ContextRepository,
    pub tasks: TaskRepository,
    pub artifacts: ArtifactRepository,
    pub execution_steps: ExecutionStepRepository,
}

impl std::fmt::Debug for WebhookRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookRepositories")
            .finish_non_exhaustive()
    }
}

/// Router-scoped state of the webhook broadcasts.
#[derive(Clone)]
pub struct WebhookState {
    pub(crate) repositories: Arc<WebhookRepositories>,
    pub(crate) events: Arc<EventRouter>,
}

impl std::fmt::Debug for WebhookState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookState").finish_non_exhaustive()
    }
}

impl WebhookState {
    pub fn from_context(ctx: &AppContext) -> Result<Self, MissingDependency> {
        let db = ctx.db_pool();
        let sessions = ctx.kernel().state().get::<DynSessionUsageCounters>()?;
        Ok(Self {
            repositories: Arc::new(crate::repository::webhook(db, Arc::clone(sessions))),
            events: Arc::new(ctx.event_router().clone()),
        })
    }
}

pub fn router(state: WebhookState) -> Router {
    Router::new()
        .route("/broadcast", post(broadcast_context_event))
        .route("/agui", post(broadcast_agui_event))
        .route("/a2a", post(broadcast_a2a_event))
        .with_state(state)
}
