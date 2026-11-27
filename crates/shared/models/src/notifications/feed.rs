//! The reader's notification feed on the bridge routes
//! (`/v1/bridge/notifications`): a newest-first page, mark read and seen
//! receipts.
//!
//! The routes take the bridge JWT as bearer. A feed page is newest first; its
//! `next_cursor` continues with older items when sent back as `since`. The
//! instance clamps a page to 1..=`FEED_PAGE_MAX` items. The bodies are the
//! types of [`crate::notifications`]; `SeenAnswer` is the one answer the
//! platform served without a shared type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

pub const FEED_PAGE_MAX: u32 = 100;

pub const FEED_QUERY_SINCE: &str = "since";

pub const FEED_QUERY_LIMIT: &str = "limit";

pub const FEED_QUERY_UNREAD: &str = "unread";

/// The answer to a seen receipt: how many of the listed notifications the
/// instance newly recorded as seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeenAnswer {
    pub seen: u64,
}
