//! The reader's notifications from the applied instance, as an extension.
//!
//! [`NotificationsExtension`] (id `notifications`) follows the applied
//! instance while it enables the extension and withholds everything
//! otherwise: no instance, or one that does not list the extension, leaves the
//! inbox empty with [`FeedStatus::Withheld`] and refuses every action. Its
//! [`NotificationsService`] polls the bridge feed (`/v1/bridge/notifications`)
//! newest first, backing off on failure, and the [`NotificationsHandle`] it
//! shares through `SpContext::handles` carries the inbox, its change feed and
//! the reader's actions: mark read, mark seen and load older items.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;
mod error;
mod extension;
mod feed;
mod handle;
mod http;
mod inbox;
mod service;

pub use client::{FeedQuery, NotificationsClient};
pub use error::NotificationsError;
pub use extension::NotificationsExtension;
pub use handle::NotificationsHandle;
pub use inbox::{FeedStatus, INBOX_CAPACITY, Inbox, UnreadCount};
pub use service::{NotificationsService, PollCadence};
pub use systemprompt_identifiers::NotificationId;
pub use systemprompt_models::api::KeysetCursor;
pub use systemprompt_models::notifications::{
    Attention, Notification, NotificationKind, SubjectRef,
};
