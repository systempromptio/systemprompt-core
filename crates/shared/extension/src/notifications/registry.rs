//! Compile-time registrations, discovery functions and registration macros
//! for the extension-contributed notification seams.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::traits::{
    NotificationAudienceResolver, NotificationChannel, NotificationRouter, PublishPolicy,
    ResourceVisibility,
};
use crate::seam::seam_registration;

seam_registration!(
    NotificationRouterRegistration,
    NotificationRouter,
    discover_notification_routers
);
seam_registration!(
    NotificationAudienceRegistration,
    NotificationAudienceResolver,
    discover_notification_audiences
);
seam_registration!(
    PublishPolicyRegistration,
    PublishPolicy,
    discover_notification_publish_policies
);
seam_registration!(
    NotificationChannelRegistration,
    NotificationChannel,
    discover_notification_channels
);
seam_registration!(
    ResourceVisibilityRegistration,
    ResourceVisibility,
    discover_resource_visibilities
);

#[doc(hidden)]
#[macro_export]
macro_rules! __register_notification_seam {
    ($($args:tt)*) => {
        $crate::__register_seam!(notifications, $($args)*);
    };
}

#[macro_export]
macro_rules! register_notification_router {
    ($($args:tt)*) => {
        $crate::__register_notification_seam!(
            NotificationRouterRegistration, NotificationRouter, $($args)*
        );
    };
}

#[macro_export]
macro_rules! register_notification_audience {
    ($($args:tt)*) => {
        $crate::__register_notification_seam!(
            NotificationAudienceRegistration, NotificationAudienceResolver, $($args)*
        );
    };
}

#[macro_export]
macro_rules! register_notification_publish_policy {
    ($($args:tt)*) => {
        $crate::__register_notification_seam!(
            PublishPolicyRegistration, PublishPolicy, $($args)*
        );
    };
}

#[macro_export]
macro_rules! register_notification_channel {
    ($($args:tt)*) => {
        $crate::__register_notification_seam!(
            NotificationChannelRegistration, NotificationChannel, $($args)*
        );
    };
}

#[macro_export]
macro_rules! register_resource_visibility {
    ($($args:tt)*) => {
        $crate::__register_notification_seam!(
            ResourceVisibilityRegistration, ResourceVisibility, $($args)*
        );
    };
}
