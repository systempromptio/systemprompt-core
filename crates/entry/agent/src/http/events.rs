//! [`ContextEventPublisher`]: the context, task and artifact events an agent
//! route publishes to one user's streams.
//!
//! The agent member builds it in its provide fn from the kernel's event
//! router; route state holds only this handle, which publishes to the user a
//! call names and cannot subscribe to, read or enumerate any stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_events::EventRouter;
use systemprompt_identifiers::UserId;
use systemprompt_models::{A2AEvent, AgUiEvent, SystemEvent};

/// Publishes events to one user's context streams.
#[derive(Clone)]
pub struct ContextEventPublisher {
    router: Arc<EventRouter>,
}

impl std::fmt::Debug for ContextEventPublisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextEventPublisher").finish_non_exhaustive()
    }
}

impl ContextEventPublisher {
    #[must_use]
    pub const fn new(router: Arc<EventRouter>) -> Self {
        Self { router }
    }

    pub async fn agui(&self, user_id: &UserId, event: AgUiEvent) -> (usize, usize) {
        self.router
            .route_agui(user_id, event)
            .await
            .into_local_logged()
    }

    pub async fn a2a(&self, user_id: &UserId, event: A2AEvent) -> (usize, usize) {
        self.router
            .route_a2a(user_id, event)
            .await
            .into_local_logged()
    }

    pub async fn system(&self, user_id: &UserId, event: SystemEvent) -> usize {
        self.router
            .route_system(user_id, event)
            .await
            .into_local_logged()
    }
}
