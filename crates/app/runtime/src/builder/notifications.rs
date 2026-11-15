//! Composition of the [`NotificationsHandle`] from the context's handles.
//!
//! The outbox sink exists before the document store, which emits through it.
//! The contributed seams are the built [`NotificationSeams`] of the
//! `notifications` capability's slot; the builder assembles the handle from
//! one [`NotificationsCore`] with those seams ahead of the core defaults (the
//! default router, the user-role audience resolver, the document and issue
//! visibilities, the AG-UI channel and the default publish policy).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::DynNotificationSink;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_notifications::channels::AguiChannel;
use systemprompt_notifications::consumer::NotificationsConsumer;
use systemprompt_notifications::defaults::{DefaultPublishPolicy, DefaultRouter};
use systemprompt_notifications::pipeline::{NotificationPipeline, PipelineParts};
use systemprompt_notifications::service::{
    EscalationDelivery, NotificationsService, NotificationsServiceParts,
};
use systemprompt_notifications::sink::OutboxNotificationSink;
use systemprompt_notifications::{NotificationRepositories, NotificationSeams};
use systemprompt_users::UserRepository;

use crate::documents::DocumentsRuntime;
use crate::notifications::{
    DocumentVisibility, IssueVisibility, NotificationsHandle, UserRoleAudienceResolver,
};

pub fn notification_sink(database: &DbPool, instance_id: &InstanceId) -> DynNotificationSink {
    OutboxNotificationSink::over_write_pool(database, instance_id)
}

pub fn core_notifications(inputs: &NotificationsInputs<'_>) -> Arc<NotificationsHandle> {
    NotificationsCore::new(inputs).assemble(&NotificationSeams::default())
}

pub(crate) struct NotificationsCore {
    database: DbPool,
    router: Arc<EventRouter>,
    sink: DynNotificationSink,
    listener_pool: PgPool,
    documents: Arc<DocumentsRuntime>,
    users: Arc<UserRepository>,
    repos: Arc<NotificationRepositories>,
    profile: NotificationsProfile,
    escalation: EscalationDelivery,
}

/// The context handles a [`NotificationsHandle`] is assembled from.
#[derive(Clone, Copy)]
pub struct NotificationsInputs<'a> {
    pub database: &'a DbPool,
    pub event_router: &'a EventRouter,
    pub sink: &'a DynNotificationSink,
    pub listener_pool: &'a PgPool,
    pub documents: &'a Arc<DocumentsRuntime>,
    pub profile: &'a NotificationsProfile,
    pub escalation: EscalationDelivery,
}

impl fmt::Debug for NotificationsInputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationsInputs")
            .field("profile", self.profile)
            .finish_non_exhaustive()
    }
}

impl NotificationsCore {
    pub(crate) fn new(inputs: &NotificationsInputs<'_>) -> Self {
        Self {
            database: Arc::clone(inputs.database),
            router: Arc::new(inputs.event_router.clone()),
            sink: Arc::clone(inputs.sink),
            listener_pool: inputs.listener_pool.clone(),
            documents: Arc::clone(inputs.documents),
            users: Arc::new(UserRepository::new(inputs.database)),
            repos: Arc::new(NotificationRepositories::new(inputs.database)),
            profile: *inputs.profile,
            escalation: inputs.escalation,
        }
    }

    pub(crate) fn assemble(&self, seams: &NotificationSeams) -> Arc<NotificationsHandle> {
        let NotificationSeams {
            mut routers,
            mut channels,
            mut audiences,
            mut publish_policies,
            mut visibilities,
        } = seams.clone();
        routers.push(Arc::new(DefaultRouter));
        audiences.push(Arc::new(UserRoleAudienceResolver::new(Arc::clone(
            &self.users,
        ))));
        visibilities.push(Arc::new(DocumentVisibility::new(
            Arc::clone(self.documents.access()),
            Arc::clone(self.documents.repositories()),
        )));
        visibilities.push(Arc::new(IssueVisibility::new(
            Arc::clone(self.documents.issues()),
            Arc::clone(&self.users),
        )));
        channels.push(Arc::new(AguiChannel::new(Arc::clone(&self.router))));
        publish_policies.push(Arc::new(DefaultPublishPolicy::new(
            audiences.clone(),
            Duration::from_millis(self.profile.consumer.resolver_timeout_ms),
        )));
        let pipeline = Arc::new(NotificationPipeline::new(PipelineParts {
            routers,
            audiences,
            visibilities,
            channels,
            repos: Arc::clone(&self.repos),
            profile: self.profile,
        }));
        let service = Arc::new(NotificationsService::new(NotificationsServiceParts {
            repos: Arc::clone(&self.repos),
            router: Arc::clone(&self.router),
            sink: Arc::clone(&self.sink),
            pipeline: Arc::clone(&pipeline),
            publish_policies,
            db: Arc::clone(&self.database),
            escalation: self.escalation,
        }));
        let consumer = Arc::new(NotificationsConsumer::new(
            pipeline,
            self.database.write_pool().as_ref().clone(),
            self.listener_pool.clone(),
            self.profile.consumer,
        ));
        Arc::new(NotificationsHandle {
            service,
            sink: Arc::clone(&self.sink),
            consumer,
        })
    }
}
