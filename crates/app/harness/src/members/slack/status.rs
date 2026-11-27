//! What the desktop can say about Slack for this reader.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sp_notifications::FeedStatus;

/// Slack as the reader's notification feed shows it.
///
/// The instance exposes no per-user Slack link or enablement state, so this
/// is derived only from the notifications the inbox holds (the newest
/// [`INBOX_CAPACITY`](sp_notifications::INBOX_CAPACITY)). `Withheld` while
/// the instance does not enable `slack` and `notifications`; `Unknown`
/// while the feed is not live, carrying its status; `Quiet` when the live
/// inbox holds no Slack-originated item; `Active` otherwise, with how many it
/// holds, how many are unread and when the newest was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlackStatus {
    Withheld,
    Unknown {
        feed: FeedStatus,
    },
    Quiet,
    Active {
        items: usize,
        unread: usize,
        latest_at: DateTime<Utc>,
    },
}
