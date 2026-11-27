//! Catch-up: the newest-first walk that brings the inbox up to date.
//!
//! Every catch-up starts at the newest notification and pages down until a
//! whole page is ids the inbox already holds, the feed ends, or
//! [`INBOX_CAPACITY`] items were fetched. A fetched item replaces the held
//! copy with its id, so a row the instance folded in place is refreshed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;

use crate::members::notifications::client::FeedQuery;
use crate::members::notifications::error::NotificationsError;
use crate::members::notifications::handle::{NotificationsHandle, Target};
use crate::members::notifications::inbox::{FeedStatus, INBOX_CAPACITY, Inbox};

pub(crate) const FEED_PAGE: u32 = 50;

pub(crate) async fn catch_up(
    handle: &NotificationsHandle,
    target: &Target,
) -> Result<(), NotificationsError> {
    let generation = target.generation;
    let mut query = FeedQuery::newest(FEED_PAGE);
    let mut fetched = 0usize;
    loop {
        let page = target.client.feed(&query).await?;
        let Some(merged) = handle.update(generation, |inbox| inbox.merge_newest(page)) else {
            return Ok(());
        };
        fetched = fetched.saturating_add(merged.fetched);
        if merged.reached_known {
            break;
        }
        match merged.next_cursor {
            None => {
                handle.update(generation, Inbox::mark_end_reached);
                break;
            },
            Some(next) if fetched < INBOX_CAPACITY => {
                query = FeedQuery::newest(FEED_PAGE).since(next);
            },
            Some(_) => break,
        }
    }
    handle.update(generation, |inbox| {
        inbox.set_status(FeedStatus::Live {
            refreshed_at: Utc::now(),
        });
    });
    Ok(())
}
