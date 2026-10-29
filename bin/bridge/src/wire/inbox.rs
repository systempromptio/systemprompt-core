//! The inbox pane's payload and the resolution of a notification's
//! site-relative `open_url` against the configured gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_models::notifications::Notification;

use crate::proxy::notifications::InboxState;
use crate::wire::external_url::{ExternalUrl, ExternalUrlRejected};

/// The `inbox.list` reply and the `inbox.changed` event body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InboxPayload {
    pub unread: i64,
    pub unread_capped: bool,
    pub has_older: bool,
    pub items: Vec<Notification>,
    pub can_start_session: bool,
    pub available: bool,
    pub last_error: Option<String>,
}

impl InboxPayload {
    #[must_use]
    pub fn new(
        state: InboxState,
        available: bool,
        last_error: Option<String>,
        can_start_session: bool,
    ) -> Self {
        Self {
            unread: state.unread,
            unread_capped: state.unread_capped,
            has_older: state.older_cursor.is_some(),
            items: state.items,
            can_start_session,
            available,
            last_error,
        }
    }
}

// Why: `open_url` is site-relative; a protocol-relative (`//host`) or
// backslashed path would let the server's string pick another host.
pub fn site_url(base: &str, open_url: &str) -> Result<ExternalUrl, ExternalUrlRejected> {
    let relative =
        open_url.starts_with('/') && !open_url.starts_with("//") && !open_url.contains('\\');
    if !relative {
        return Err(ExternalUrlRejected::Malformed(open_url.to_owned()));
    }
    let joined = url::Url::parse(base)
        .and_then(|base| base.join(open_url))
        .map_err(|_source| ExternalUrlRejected::Malformed(open_url.to_owned()))?;
    ExternalUrl::parse(joined.as_str())
}
