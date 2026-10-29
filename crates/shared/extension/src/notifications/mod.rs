//! Notification seams: the outbox sink, routers, audience resolvers, publish
//! policies, resource visibility and delivery channels.
//!
//! Producers emit through the injected [`NotificationSink`] inside their own
//! transaction. Extensions contribute the other five seams with the
//! `register_notification_*!`/`register_resource_visibility!` macros; each
//! registration carries a `priority` and a factory over the
//! [`ExtensionContext`], and the matching `discover_*` function returns them
//! sorted by ascending priority. Every trait is held as `Arc<dyn _>`, so each
//! uses `#[async_trait]`; native `async fn` in traits is not `dyn`-compatible.
//!
//! Audience resolvers also answer [`NotificationAudienceResolver::is_member`]
//! with a [`Membership`] so a subscription can be checked against its
//! audience without expanding it; [`ResourceVisibility::visible_many`] decides
//! one resource for many readers in a single call. The sink and the
//! `Database` error variants need the `sqlx` feature.
//!
//! These traits live here rather than in `systemprompt_traits` because they
//! name `systemprompt_models::notifications` types, and the models crate
//! already depends on the traits crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod errors;
mod registry;
mod traits;

pub use errors::{
    ChannelError, NotificationAudienceError, NotificationRouterError, NotificationSinkError,
    PublishPolicyError, VisibilityError,
};
pub use registry::{
    NotificationAudienceRegistration, NotificationChannelRegistration,
    NotificationRouterRegistration, PublishPolicyRegistration, ResourceVisibilityRegistration,
    discover_notification_audiences, discover_notification_channels,
    discover_notification_publish_policies, discover_notification_routers,
    discover_resource_visibilities,
};
#[cfg(feature = "sqlx")]
pub use traits::{DynNotificationSink, NotificationSink};
pub use traits::{
    Membership, NotificationAudienceResolver, NotificationChannel, NotificationRouter,
    PublishPolicy, PublishVerdict, ResourceVisibility, Visibility,
};
