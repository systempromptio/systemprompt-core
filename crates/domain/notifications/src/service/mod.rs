//! [`NotificationsService`]: the notifications API every surface calls.
//!
//! Every method takes the calling user and acts only on that user's rows,
//! subscriptions and rules. Realtime events (`notification.read`,
//! `notification.escalated`) go to the caller's AG-UI stream after the change
//! commits; a failed relay to other replicas is logged and the committed
//! change stands, because local delivery has already happened and every
//! client re-reads its feed on reconnect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod feed;
mod publish;
mod rules;
mod streams;
mod subscriptions;

use std::sync::Arc;

use serde::Serialize;
use systemprompt_database::DbPool;
use systemprompt_events::{EventRouter, RelayOutcome};
use systemprompt_extension::notifications::{DynNotificationSink, PublishPolicy};
use systemprompt_identifiers::UserId;
use systemprompt_models::agui::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};
use systemprompt_models::notifications::{SimulateRequest, SimulateResponse};

pub use feed::{FeedQuery, MarkReadOutcome};
pub use publish::{PublishOutcome, PublishRequest};

pub use crate::models::SubscriptionView;

use crate::error::NotificationsResult;
use crate::pipeline::NotificationPipeline;
use crate::repository::NotificationRepositories;

/// Everything [`NotificationsService`] is built from.
#[derive(Clone)]
pub struct NotificationsServiceParts {
    pub repos: Arc<NotificationRepositories>,
    pub router: Arc<EventRouter>,
    pub sink: DynNotificationSink,
    pub pipeline: Arc<NotificationPipeline>,
    pub publish_policies: Vec<Arc<dyn PublishPolicy>>,
    pub db: DbPool,
}

impl std::fmt::Debug for NotificationsServiceParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsServiceParts")
            .field("publish_policies", &self.publish_policies.len())
            .finish_non_exhaustive()
    }
}

/// The notifications read and write API.
#[derive(Clone)]
pub struct NotificationsService {
    repos: Arc<NotificationRepositories>,
    router: Arc<EventRouter>,
    sink: DynNotificationSink,
    pipeline: Arc<NotificationPipeline>,
    publish_policies: Vec<Arc<dyn PublishPolicy>>,
    db: DbPool,
}

impl std::fmt::Debug for NotificationsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsService")
            .field("publish_policies", &self.publish_policies.len())
            .finish_non_exhaustive()
    }
}

impl NotificationsService {
    #[must_use]
    pub fn new(parts: NotificationsServiceParts) -> Self {
        Self {
            repos: parts.repos,
            router: parts.router,
            sink: parts.sink,
            pipeline: parts.pipeline,
            publish_policies: parts.publish_policies,
            db: parts.db,
        }
    }

    #[must_use]
    pub fn sink(&self) -> DynNotificationSink {
        Arc::clone(&self.sink)
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<NotificationRepositories> {
        &self.repos
    }

    pub async fn simulate(
        &self,
        request: SimulateRequest,
    ) -> NotificationsResult<SimulateResponse> {
        let SimulateRequest { event } = request;
        self.pipeline.simulate(&event).await
    }

    async fn emit<T: Serialize + Sync>(
        &self,
        user: &UserId,
        name: &'static str,
        payload: &T,
    ) -> NotificationsResult<()> {
        let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
            name: name.to_owned(),
            value: serde_json::to_value(payload)?,
        }));
        if let RelayOutcome::Failed(error) = self.router.route_agui(user, event).await.relay {
            tracing::warn!(
                event = name,
                user_id = %user,
                error = %error,
                "Notification event delivered locally but not relayed to other replicas"
            );
        }
        Ok(())
    }
}
