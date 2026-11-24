//! The notification sink of a server built without the `notifications`
//! feature: it reports itself unavailable, so the document store commits
//! comments and grants without recording an event and refuses an explicit
//! mention attention choice.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{NotificationSink, NotificationSinkError};
use systemprompt_models::notifications::NotificationEvent;

/// A [`NotificationSink`] that records nothing and says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct NotificationsUnavailableSink;

#[async_trait::async_trait]
impl NotificationSink for NotificationsUnavailableSink {
    fn is_available(&self) -> bool {
        false
    }

    async fn emit(
        &self,
        _conn: &mut sqlx::PgConnection,
        _event: &NotificationEvent,
    ) -> Result<(), NotificationSinkError> {
        Err(NotificationSinkError::Unavailable)
    }
}
