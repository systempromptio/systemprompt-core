//! The `register_*!` macros for the notification seams. No registry collects
//! their submissions; the seams are contributed through the owning
//! capability's slot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
