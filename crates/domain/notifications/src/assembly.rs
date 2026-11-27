//! The core defaults of the built [`NotificationSeams`] and the service and
//! outbox consumer assembled over them.
//!
//! After the contributed seams, the slot build appends the core defaults:
//! the [`DefaultRouter`], the document and issue visibilities the `files`
//! member provides, the [`AguiChannel`] over the kernel's event router and the
//! [`DefaultPublishPolicy`] over every audience resolver. After the AG-UI
//! channel come the push channels `services/notifications/push.yaml`
//! declares and the secret store holds a credential for; they need the
//! push token directory the device-owning member provides, and an instance
//! with no declaration, no credential or no directory gets none. The audience
//! resolvers are the kernel-built [`AudienceResolvers`] set, shared with
//! brain recall; without it the build is refused. A default whose
//! owner is not admitted is absent, never replaced. The
//! [`NotificationsService`] and [`NotificationsConsumer`] are then built
//! from those seams and bound into the cells of the
//! [`NotificationsServiceSlot`] the build creates and returns, so every reader
//! sees the one pipeline the kernel resolved and nothing outside the build can
//! bind either cell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::notifications::DynPushTokenDirectory;
use systemprompt_extension::{
    AudienceResolvers, DynNotificationSink, Resolved, ResourceVisibility, SlotError,
};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_traits::core_seam::{DocumentVisibility, IssueVisibility};
use systemprompt_traits::{CoreSeam, SecretStore};

use crate::channels::AguiChannel;
use crate::channels::push::{PushParts, ReqwestProviderHttp, StreamPresence, push_channels};
use crate::consumer::NotificationsConsumer;
use crate::contract::NotificationSeams;
use crate::defaults::{DefaultPublishPolicy, DefaultRouter};
use crate::pipeline::{NotificationPipeline, PipelineParts};
use crate::plane::push::PushDeclaration;
use crate::repository::NotificationRepositories;
use crate::service::{NotificationsService, NotificationsServiceParts, NotificationsServiceSlot};
use systemprompt_traits::EscalationDelivery;

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
    append_push_channels(seams, r)?;
    let profile = r.get::<Arc<NotificationsProfile>>()?;
    seams
        .publish_policies
        .push(Arc::new(DefaultPublishPolicy::new(
            seams.audiences.clone(),
            Duration::from_millis(profile.consumer.resolver_timeout_ms),
        )));
    Ok(())
}

fn append_push_channels(seams: &mut NotificationSeams, r: &Resolved<'_>) -> Result<(), SlotError> {
    let Some(root) = r.optional::<ActiveServicesRoot>() else {
        return Ok(());
    };
    let Some(declaration) = PushDeclaration::load(&root.path).map_err(refused)? else {
        return Ok(());
    };
    if declaration.is_empty() {
        return Ok(());
    }
    let (Some(secrets), Some(directory)) = (
        r.optional::<Arc<dyn SecretStore>>(),
        r.optional::<DynPushTokenDirectory>(),
    ) else {
        tracing::warn!(
            "Push is declared but this host has no secret store or push token directory; no push \
             channel is contributed"
        );
        return Ok(());
    };
    let parts = PushParts {
        secrets,
        directory,
        presence: Arc::new(StreamPresence::new(r.get::<Arc<EventRouter>>()?)),
        http: Arc::new(ReqwestProviderHttp::build().map_err(refused)?),
    };
    seams
        .channels
        .extend(push_channels(&declaration, &parts).map_err(refused)?);
    Ok(())
}

pub(crate) fn bind_service(
    seams: NotificationSeams,
    r: &Resolved<'_>,
) -> Result<NotificationsServiceSlot, SlotError> {
    let (slot, binder) = NotificationsServiceSlot::unbound(seams);
    let seams = slot.seams();
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
        db.notifications().clone(),
        profile.consumer,
    ));
    binder.bind(service, consumer).map_err(refused)?;
    Ok(slot)
}

fn refused(error: impl std::error::Error + Send + Sync + 'static) -> SlotError {
    SlotError::Refused {
        slot: SLOT,
        source: Box::new(error),
    }
}
