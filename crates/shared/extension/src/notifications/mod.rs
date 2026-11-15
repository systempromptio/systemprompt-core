//! Notification seams: the outbox sink, routers, audience resolvers, publish
//! policies, resource visibility and delivery channels.
//!
//! Producers emit through the injected `NotificationSink` inside their own
//! transaction. Extensions contribute the other five seams through the
//! notifications capability's slot, as factories over `Resolved`. Every trait
//! is held as `Arc<dyn _>`, so each uses `#[async_trait]`; native `async fn` in
//! traits is not `dyn`-compatible.
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
#[cfg(feature = "sqlx")]
pub use traits::{DynNotificationSink, NotificationSink};
pub use traits::{
    Membership, NotificationAudienceResolver, NotificationChannel, NotificationRouter,
    PublishPolicy, PublishVerdict, ResourceVisibility, Visibility,
};
