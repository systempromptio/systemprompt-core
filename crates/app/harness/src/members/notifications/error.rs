//! Why a notifications call, or a poll of the feed, failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use sp_client::ClientError;
use sp_protocol::view::SyncErrorKind;

/// A failure of the notifications feed or of one of the reader's actions.
///
/// `Withheld` is the fail-closed answer: no instance is applied, or it does
/// not enable the extension. `NotServed` means the instance answered 404, so
/// it serves no notifications feed at all.
#[derive(Debug, thiserror::Error)]
pub enum NotificationsError {
    #[error("notifications are withheld: no instance enables them")]
    Withheld,
    #[error("{endpoint}: the instance serves no notifications feed")]
    NotServed { endpoint: &'static str },
    #[error("{endpoint}: the instance refused the request with status {status}")]
    Rejected { endpoint: &'static str, status: u16 },
    #[error("{endpoint}: the answer does not decode")]
    Decode {
        endpoint: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("the request could not be built")]
    Encode(#[source] serde_json::Error),
    #[error("the notifications call failed")]
    Client(#[from] ClientError),
}

impl NotificationsError {
    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Client(error) => error.retry_after(),
            _ => None,
        }
    }

    #[must_use]
    pub fn kind(&self) -> SyncErrorKind {
        match self {
            Self::Withheld => SyncErrorKind::Refused,
            Self::NotServed { .. } => SyncErrorKind::Service,
            Self::Rejected { .. } | Self::Decode { .. } | Self::Encode(_) => SyncErrorKind::Invalid,
            Self::Client(error) => error.sync_error_kind(),
        }
    }
}
