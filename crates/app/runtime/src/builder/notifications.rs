//! Composition of the [`NotificationsHandle`] from the context's handles.
//!
//! The outbox sink exists before the document store, which emits through it.
//! Extension seam factories take the built context, so the builder assembles
//! the handle twice from one [`NotificationsCore`]: first with core seams
//! alone, to complete the context the factories are handed, then with the
//! discovered extension seams ahead of the core defaults, replacing the
//! first before the context is returned.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_events::services::durable::DurableOutbox;
use systemprompt_extension::{
    DynNotificationSink, ExtensionContext, NotificationAudienceResolver, NotificationChannel,
    NotificationRouter, PublishPolicy, ResourceVisibility, discover_notification_audiences,
    discover_notification_channels, discover_notification_publish_policies,
    discover_notification_routers, discover_resource_visibilities,
};
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_notifications::NotificationRepositories;
use systemprompt_notifications::channels::AguiChannel;
use systemprompt_notifications::consumer::NotificationsConsumer;
use systemprompt_notifications::defaults::{DefaultPublishPolicy, DefaultRouter};
use systemprompt_notifications::pipeline::{NotificationPipeline, PipelineParts};
use systemprompt_notifications::service::{NotificationsService, NotificationsServiceParts};
use systemprompt_notifications::sink::OutboxNotificationSink;
use systemprompt_users::UserRepository;

use crate::documents::DocumentsRuntime;
use crate::notifications::{
    DocumentVisibility, IssueVisibility, NotificationsHandle, UserRoleAudienceResolver,
};

pub fn notification_sink(database: &DbPool, instance_id: InstanceId) -> DynNotificationSink {
    let outbox = DurableOutbox::new(database.write_pool().as_ref().clone(), instance_id);
    Arc::new(OutboxNotificationSink::new(Arc::new(outbox)))
}

pub fn core_notifications(inputs: &NotificationsInputs<'_>) -> Arc<NotificationsHandle> {
    NotificationsCore::new(inputs).assemble(ExtensionSeams::default())
}

#[derive(Default)]
pub(super) struct ExtensionSeams {
    pub(super) routers: Vec<Arc<dyn NotificationRouter>>,
    pub(super) audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub(super) visibilities: Vec<Arc<dyn ResourceVisibility>>,
    pub(super) channels: Vec<Arc<dyn NotificationChannel>>,
    pub(super) publish_policies: Vec<Arc<dyn PublishPolicy>>,
}

impl ExtensionSeams {
    pub(super) fn discover(ctx: &dyn ExtensionContext) -> Self {
        Self {
            routers: discover_notification_routers()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            audiences: discover_notification_audiences()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            visibilities: discover_resource_visibilities()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            channels: discover_notification_channels()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
            publish_policies: discover_notification_publish_policies()
                .into_iter()
                .map(|registration| (registration.factory)(ctx))
                .collect(),
        }
    }
}

pub(super) struct NotificationsCore {
    database: DbPool,
    router: Arc<EventRouter>,
    sink: DynNotificationSink,
    listener_pool: PgPool,
    documents: Arc<DocumentsRuntime>,
    users: Arc<UserRepository>,
    repos: Arc<NotificationRepositories>,
    profile: NotificationsProfile,
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
}

impl fmt::Debug for NotificationsInputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationsInputs")
            .field("profile", self.profile)
            .finish_non_exhaustive()
    }
}

impl NotificationsCore {
    pub(super) fn new(inputs: &NotificationsInputs<'_>) -> Self {
        Self {
            database: Arc::clone(inputs.database),
            router: Arc::new(inputs.event_router.clone()),
            sink: Arc::clone(inputs.sink),
            listener_pool: inputs.listener_pool.clone(),
            documents: Arc::clone(inputs.documents),
            users: Arc::new(UserRepository::new(inputs.database)),
            repos: Arc::new(NotificationRepositories::new(inputs.database)),
            profile: *inputs.profile,
        }
    }

    pub(super) fn assemble(&self, seams: ExtensionSeams) -> Arc<NotificationsHandle> {
        let ExtensionSeams {
            mut routers,
            mut audiences,
            mut visibilities,
            mut channels,
            mut publish_policies,
        } = seams;
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
            actors: Arc::new(UserRoleAudienceResolver::new(Arc::clone(&self.users))),
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
