//! [`NotificationsSlot`]: the notification pipeline's contribution point.
//!
//! A member contributes a router, channel, audience resolver, publish policy
//! or resource visibility with `c.slot::<NotificationsSlot>()` and the
//! matching builder method; each method stores a factory and runs nothing.
//! Admission drops a disabled member's builder before any factory runs, and
//! the kernel then builds the slot once: every factory is called against the
//! resolved state, in composition order, and the result is the
//! [`NotificationSeams`] value the kernel inserts as
//! `Arc<NotificationSeams>`. The `notifications` member always opens the
//! slot, so the built seams are present exactly when that member is admitted.
//!
//! The seams are the contributed ones only. The composition root that
//! assembles the pipeline appends the core defaults (the default router, the
//! AG-UI channel, the user-role audience resolver, the document and issue
//! visibilities and the default publish policy) after them, so a contributed
//! seam is consulted first. A factory that fails, or reads a value nothing
//! provides, refuses the build: a seam is never silently left out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_extension::{
    Factory, NotificationAudienceResolver, NotificationChannel, NotificationRouter, PublishPolicy,
    Resolved, ResourceVisibility, Slot, SlotEntry, SlotError, SlotOrder, build_factories,
};

const SLOT: &str = "notifications";

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// The notification pipeline's contribution point, owned by the
/// `notifications` capability.
#[derive(Debug, Clone, Copy)]
pub struct NotificationsSlot;

/// One member's notification seams, stored until the slot is built.
#[derive(Default)]
pub struct NotificationsSlotBuilder {
    routers: Factories<dyn NotificationRouter>,
    channels: Factories<dyn NotificationChannel>,
    audiences: Factories<dyn NotificationAudienceResolver>,
    publish_policies: Factories<dyn PublishPolicy>,
    visibilities: Factories<dyn ResourceVisibility>,
}

impl NotificationsSlotBuilder {
    pub fn router<F: Factory<Arc<dyn NotificationRouter>>>(&mut self, f: F) -> &mut Self {
        self.routers.push(Box::new(f));
        self
    }

    pub fn channel<F: Factory<Arc<dyn NotificationChannel>>>(&mut self, f: F) -> &mut Self {
        self.channels.push(Box::new(f));
        self
    }

    pub fn audience<F: Factory<Arc<dyn NotificationAudienceResolver>>>(
        &mut self,
        f: F,
    ) -> &mut Self {
        self.audiences.push(Box::new(f));
        self
    }

    pub fn publish_policy<F: Factory<Arc<dyn PublishPolicy>>>(&mut self, f: F) -> &mut Self {
        self.publish_policies.push(Box::new(f));
        self
    }

    pub fn resource_visibility<F: Factory<Arc<dyn ResourceVisibility>>>(
        &mut self,
        f: F,
    ) -> &mut Self {
        self.visibilities.push(Box::new(f));
        self
    }
}

impl fmt::Debug for NotificationsSlotBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationsSlotBuilder")
            .field("routers", &self.routers.len())
            .field("channels", &self.channels.len())
            .field("audiences", &self.audiences.len())
            .field("publish_policies", &self.publish_policies.len())
            .field("visibilities", &self.visibilities.len())
            .finish()
    }
}

/// The built notification seams, in composition order.
#[derive(Clone, Default)]
pub struct NotificationSeams {
    pub routers: Vec<Arc<dyn NotificationRouter>>,
    pub channels: Vec<Arc<dyn NotificationChannel>>,
    pub audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
    pub publish_policies: Vec<Arc<dyn PublishPolicy>>,
    pub visibilities: Vec<Arc<dyn ResourceVisibility>>,
}

impl fmt::Debug for NotificationSeams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotificationSeams")
            .field("routers", &self.routers.len())
            .field("channels", &self.channels.len())
            .field("audiences", &self.audiences.len())
            .field("publish_policies", &self.publish_policies.len())
            .field("visibilities", &self.visibilities.len())
            .finish()
    }
}

impl Slot for NotificationsSlot {
    type Builder = NotificationsSlotBuilder;
    type Built = NotificationSeams;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut seams = NotificationSeams::default();
        for entry in entries {
            let owner = &entry.owner;
            let builder = entry.builder;
            seams
                .routers
                .extend(build_factories(SLOT, owner, &builder.routers, r)?);
            seams
                .channels
                .extend(build_factories(SLOT, owner, &builder.channels, r)?);
            seams
                .audiences
                .extend(build_factories(SLOT, owner, &builder.audiences, r)?);
            seams.publish_policies.extend(build_factories(
                SLOT,
                owner,
                &builder.publish_policies,
                r,
            )?);
            seams
                .visibilities
                .extend(build_factories(SLOT, owner, &builder.visibilities, r)?);
        }
        Ok(seams)
    }
}
