//! Request limits, protocol revisions and the notifications a server sends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 4] = [
    LATEST_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Per-request limits: a timeout ([`DEFAULT_REQUEST_TIMEOUT`] when absent) and
/// a token that cancels the request. Either ends the wait and tells the server
/// with `notifications/cancelled`.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    pub timeout: Option<Duration>,
    pub cancel: Option<CancellationToken>,
}

impl RequestOptions {
    pub const fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
            cancel: None,
        }
    }

    pub fn cancel_on(mut self, cancel: CancellationToken) -> Self {
        self.cancel = Some(cancel);
        self
    }
}

/// A notification the server sent, such as `notifications/tools/list_changed`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerNotification {
    pub method: String,
    // JSON: a notification's `params` member is the method's own payload.
    pub params: Value,
}
