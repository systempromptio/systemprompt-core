//! Notifications: who hears about what happened, how loudly, and where.
//!
//! A producer appends a [`NotificationEvent`] to the durable outbox inside its
//! own transaction through the injected [`DynNotificationSink`]
//! ([`sink::OutboxNotificationSink`]). The [`consumer::NotificationsConsumer`]
//! claims those facts one transaction each and runs them through the
//! [`pipeline::NotificationPipeline`]: registered routers plan recipients, the
//! stream audience (a membership gate as well as a list) and subscriptions
//! add more, stream bands, rules, subscription ceilings and quiet hours set
//! each recipient's [`Attention`], resource visibility withholds what a
//! reader may not see, and every surviving recipient gets one coalesced row.
//! Rows commit as `pending` and are announced on the registered channels
//! after the commit; a timed sweep re-delivers anything a crash left
//! `pending` and releases digest-held rows, so delivery is at least once.
//! Every withheld recipient and failed step is a
//! `notification_delivery_failures` row; a failure only ever makes delivery
//! quieter or narrower, never louder or wider.
//!
//! [`service::NotificationsService`] is the read and write API the HTTP and MCP
//! surfaces call: the feed, read and escalation state, streams, publishing,
//! subscriptions, personal rules and dry-run simulation.
//!
//! Streams, declared rules and declared subscriptions come from
//! `services/notifications/*.yaml` through the [`plane::NotificationsPlane`]
//! config plane. The [`jobs::NotificationRetentionJob`] purges read rows past
//! the profile retention, rows past a stream's own retention, unread rows
//! past the unread age and old delivery failures.
//!
//! Errors are [`error::NotificationsError`]; repositories answer
//! `systemprompt_traits::RepositoryError`.
//!
//! [`NotificationEvent`]: systemprompt_models::notifications::NotificationEvent
//! [`DynNotificationSink`]: systemprompt_extension::notifications::DynNotificationSink
//! [`Attention`]: systemprompt_models::notifications::Attention
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod channels;
pub mod consumer;
pub mod defaults;
pub mod error;
pub mod extension;
pub mod jobs;
pub mod models;
pub mod pipeline;
pub mod plane;
pub mod repository;
pub mod schema;
pub mod service;
pub mod sink;

pub use channels::AguiChannel;
pub use consumer::{BacklogSnapshot, ConsumerState, NotificationsConsumer};
pub use defaults::{DefaultPublishPolicy, DefaultRouter};
pub use error::{NotificationsError, NotificationsResult};
pub use extension::NotificationsExtension;
pub use jobs::NotificationRetentionJob;
pub use models::{FeedFilter, NewNotifications, NewRecipient, PersistedRow, SubscriptionView};
pub use pipeline::{
    BatchContext, DeliverOutcome, NotificationPipeline, PipelineParts, ProcessOutcome, Withheld,
};
pub use plane::{NOTIFICATIONS_PLANE, NotificationsPlane};
pub use repository::NotificationRepositories;
pub use service::{
    FeedQuery, MarkReadOutcome, NotificationsService, NotificationsServiceParts, PublishOutcome,
    PublishRequest,
};
pub use sink::OutboxNotificationSink;
