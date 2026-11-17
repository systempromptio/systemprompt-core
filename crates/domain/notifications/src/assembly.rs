//! The core defaults of the built [`NotificationSeams`] and the service and
//! outbox consumer assembled over them.
//!
//! After the contributed seams, the slot build appends the core defaults:
//! the [`DefaultRouter`], the document and issue visibilities the `files`
//! member provides, the [`AguiChannel`] over the kernel's event router and the
//! [`DefaultPublishPolicy`] over every audience resolver. The audience
//! resolvers are the kernel-built [`AudienceResolvers`] set, shared with
//! brain recall; without it the build is refused. A default whose
//! owner is not admitted is absent, never replaced. The
//! [`NotificationsService`] and [`NotificationsConsumer`] are then built
//! from those seams and bound into the [`NotificationsServiceSlot`] and
//! [`NotificationsConsumerSlot`] this member provides, so every reader of
//! the slots sees the one pipeline the kernel resolved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::{
    AudienceResolvers, DynNotificationSink, Resolved, ResourceVisibility, SlotError,
};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_traits::CoreSeam;
use systemprompt_traits::core_seam::{DocumentVisibility, IssueVisibility};

use crate::channels::AguiChannel;
use crate::consumer::{NotificationsConsumer, NotificationsConsumerSlot};
use crate::contract::NotificationSeams;
use crate::defaults::{DefaultPublishPolicy, DefaultRouter};
use crate::pipeline::{NotificationPipeline, PipelineParts};
use crate::repository::NotificationRepositories;
use crate::service::{
    EscalationDelivery, NotificationsService, NotificationsServiceParts, NotificationsServiceSlot,
};

const SLOT: &str = "notifications";

pub(crate) fn append_core_defaults(
    seams: &mut NotificationSeams,
    r: &Resolved<'_>,
) -> Result<(), SlotError> {
    seams.audiences = r.get::<Arc<AudienceResolvers>>()?.to_vec();
    seams.routers.push(Arc::new(DefaultRouter));
    if let Some(documents) = r.optional::<CoreSeam<dyn ResourceVisibility, DocumentVisibility>>() {
        seams.visibilities.push(documents.seam());
    }
    if let Some(issues) = r.optional::<CoreSeam<dyn ResourceVisibility, IssueVisibility>>() {
        seams.visibilities.push(issues.seam());
    }
    seams
        .channels
        .push(Arc::new(AguiChannel::new(r.get::<Arc<EventRouter>>()?)));
    let profile = r.get::<Arc<NotificationsProfile>>()?;
    seams
        .publish_policies
        .push(Arc::new(DefaultPublishPolicy::new(
            seams.audiences.clone(),
            Duration::from_millis(profile.consumer.resolver_timeout_ms),
        )));
    Ok(())
}

pub(crate) fn bind_service(seams: &NotificationSeams, r: &Resolved<'_>) -> Result<(), SlotError> {
    let db = r.get::<DbPool>()?;
    let profile = *r.get::<Arc<NotificationsProfile>>()?;
    let repos = Arc::new(NotificationRepositories::new(&db));
    let pipeline = Arc::new(NotificationPipeline::new(PipelineParts {
        routers: seams.routers.clone(),
        audiences: seams.audiences.clone(),
        visibilities: seams.visibilities.clone(),
        channels: seams.channels.clone(),
        repos: Arc::clone(&repos),
        profile,
    }));
    let service = Arc::new(NotificationsService::new(NotificationsServiceParts {
        repos,
        router: r.get::<Arc<EventRouter>>()?,
        sink: r.get::<DynNotificationSink>()?,
        pipeline: Arc::clone(&pipeline),
        publish_policies: seams.publish_policies.clone(),
        db: Arc::clone(&db),
        escalation: r.get::<EscalationDelivery>()?,
    }));
    let consumer = Arc::new(NotificationsConsumer::new(
        pipeline,
        db.write_pool().as_ref().clone(),
        db.session_pool().as_ref().clone(),
        profile.consumer,
    ));
    r.get::<NotificationsServiceSlot>()?
        .bind(service)
        .map_err(refused)?;
    r.get::<NotificationsConsumerSlot>()?
        .bind(consumer)
        .map_err(refused)
}

fn refused(error: systemprompt_models::AlreadyBound) -> SlotError {
    SlotError::Refused {
        slot: SLOT,
        source: Box::new(error),
    }
}
