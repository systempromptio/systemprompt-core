//! [`ForegroundPresence`]: whether a recipient is being reached in-app.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_events::{Broadcaster, EventRouter};
use systemprompt_identifiers::UserId;

/// Whether a user has a live event stream, so the in-app channel reaches
/// them and a push would repeat it. Held as `Arc<dyn ForegroundPresence>`,
/// so it uses `#[async_trait]`.
#[async_trait]
pub trait ForegroundPresence: Send + Sync {
    async fn is_foreground(&self, user: &UserId) -> bool;
}

/// [`ForegroundPresence`] over the streams this process holds: the AG-UI and
/// context streams the in-app channel writes a notification to.
#[derive(Debug, Clone)]
pub struct StreamPresence {
    router: Arc<EventRouter>,
}

impl StreamPresence {
    #[must_use]
    pub const fn new(router: Arc<EventRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl ForegroundPresence for StreamPresence {
    async fn is_foreground(&self, user: &UserId) -> bool {
        let broadcasters = self.router.broadcasters();
        broadcasters.agui.connection_count(user).await > 0
            || broadcasters.context.connection_count(user).await > 0
    }
}
