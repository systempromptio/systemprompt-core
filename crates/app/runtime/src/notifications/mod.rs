//! Notifications composed for the runtime.
//!
//! The [`AppContext`] builder assembles one [`NotificationsHandle`]: the
//! `systemprompt_notifications` service, the outbox sink the document store
//! and other producers emit through inside their own transactions, and the
//! outbox consumer the API server runs as a supervised runner service.
//! Extension seams (routers, audience resolvers, publish policies, channels
//! and resource visibility) are discovered over the built context and the
//! core defaults are appended after them: [`UserRoleAudienceResolver`] for
//! users, roles and everyone, [`DocumentVisibility`] for documents,
//! folders and comments, and [`IssueVisibility`] for issue reports.
//!
//! [`AppContext`]: crate::AppContext
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod audience;
mod issue_visibility;
mod visibility;

use std::sync::Arc;

use systemprompt_extension::DynNotificationSink;
use systemprompt_notifications::consumer::NotificationsConsumer;
use systemprompt_notifications::service::NotificationsService;

pub use audience::UserRoleAudienceResolver;
pub use issue_visibility::IssueVisibility;
pub use visibility::DocumentVisibility;

/// The runtime's notifications service, producer sink and outbox consumer.
#[derive(Clone)]
pub struct NotificationsHandle {
    pub service: Arc<NotificationsService>,
    pub sink: DynNotificationSink,
    pub consumer: Arc<NotificationsConsumer>,
}

impl std::fmt::Debug for NotificationsHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationsHandle")
            .finish_non_exhaustive()
    }
}
