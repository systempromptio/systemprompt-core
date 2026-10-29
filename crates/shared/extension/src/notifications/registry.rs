//! Compile-time registrations, discovery functions and registration macros
//! for the extension-contributed notification seams.

use std::sync::Arc;

use super::traits::{
    NotificationAudienceResolver, NotificationChannel, NotificationRouter, PublishPolicy,
    ResourceVisibility,
};
use crate::context::ExtensionContext;

macro_rules! notification_registration {
    ($registration:ident, $trait:ident, $discover:ident) => {
        /// Compile-time registration of an extension notification seam,
        /// collected via `inventory`.
        #[derive(Debug, Clone, Copy)]
        pub struct $registration {
            pub priority: i32,
            pub factory: fn(&dyn ExtensionContext) -> Arc<dyn $trait>,
        }

        inventory::collect!($registration);

        #[must_use]
        pub fn $discover() -> Vec<&'static $registration> {
            let mut registrations: Vec<&'static $registration> =
                inventory::iter::<$registration>.into_iter().collect();
            registrations.sort_by_key(|registration| registration.priority);
            registrations
        }
    };
}

notification_registration!(
    NotificationRouterRegistration,
    NotificationRouter,
    discover_notification_routers
);
notification_registration!(
    NotificationAudienceRegistration,
    NotificationAudienceResolver,
    discover_notification_audiences
);
notification_registration!(
    PublishPolicyRegistration,
    PublishPolicy,
    discover_notification_publish_policies
);
notification_registration!(
    NotificationChannelRegistration,
    NotificationChannel,
    discover_notification_channels
);
notification_registration!(
    ResourceVisibilityRegistration,
    ResourceVisibility,
    discover_resource_visibilities
);

#[doc(hidden)]
#[macro_export]
macro_rules! __register_notification_seam {
    ($registration:ident, $trait:ident, factory = $factory:expr, priority = $priority:expr) => {
        $crate::__inventory::submit! {
            $crate::notifications::$registration {
                priority: $priority,
                factory: $factory,
            }
        }
    };
    ($registration:ident, $trait:ident, $ty:ty, priority = $priority:expr) => {
        $crate::__inventory::submit! {
            $crate::notifications::$registration {
                priority: $priority,
                factory: |_| ::std::sync::Arc::new(<$ty>::default())
                    as ::std::sync::Arc<dyn $crate::notifications::$trait>,
            }
        }
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
