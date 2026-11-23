//! [`BoundConsumerRunner`]: the `notifications` member's runner service.
//!
//! The consumer is assembled by the notifications slot build once the
//! pipeline seams are built, and bound into the [`NotificationsConsumerSlot`]
//! the build returns. The runner reads that cell when it starts and runs the
//! bound consumer; a cell still unbound when runner services start is logged
//! as an error and the runner ends, so a host that never assembled the
//! consumer is visible rather than silently draining nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_traits::{JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use super::NotificationsConsumerSlot;

#[derive(Debug, Clone)]
pub struct BoundConsumerRunner {
    slot: NotificationsConsumerSlot,
}

impl BoundConsumerRunner {
    #[must_use]
    pub const fn new(slot: NotificationsConsumerSlot) -> Self {
        Self { slot }
    }

    #[must_use]
    pub fn into_service(self) -> Arc<dyn RunnerService> {
        Arc::new(self)
    }
}

#[async_trait]
impl RunnerService for BoundConsumerRunner {
    fn name(&self) -> &'static str {
        "notifications-consumer"
    }

    async fn run(&self, ctx: JobContext, shutdown: CancellationToken) {
        if let Some(consumer) = self.slot.get() {
            consumer.run(ctx, shutdown).await;
        } else {
            tracing::error!(
                runner = "notifications-consumer",
                "notifications consumer slot is unbound; the consumer is not running"
            );
        }
    }
}
