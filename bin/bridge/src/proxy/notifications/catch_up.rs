//! Catch-up: the newest-first walk that fills what the stream missed.
//!
//! Every catch-up starts at the newest notification and pages down until a
//! page overlaps an id the inbox already holds, the feed ends, or
//! [`MAX_ITEMS`] have been fetched. The fetch is injected, so the walk is the
//! same against the gateway and against a scripted feed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::NotificationId;
use systemprompt_models::notifications::{KeysetCursor, NotificationPage};

use super::handle::InboxHandle;
use super::state::MAX_ITEMS;

pub const CATCH_UP_PAGE: u32 = 50;

pub async fn catch_up<F, Fut, E>(
    handle: &InboxHandle,
    mut fetch: F,
) -> Result<Vec<NotificationId>, E>
where
    F: FnMut(Option<KeysetCursor>, u32) -> Fut,
    Fut: Future<Output = Result<NotificationPage, E>>,
{
    let mut fresh = Vec::new();
    let mut cursor = None;
    let mut fetched = 0usize;
    loop {
        let page = fetch(cursor.take(), CATCH_UP_PAGE).await?;
        let merged = handle.update(|state| state.merge_newest(page));
        fresh.extend(merged.fresh);
        fetched += merged.fetched;
        let short = merged.fetched < CATCH_UP_PAGE as usize;
        if merged.reached_known {
            break;
        }
        if short || fetched >= MAX_ITEMS || merged.next_cursor.is_none() {
            handle.update(|state| state.older_cursor = merged.next_cursor);
            break;
        }
        cursor = merged.next_cursor;
    }
    Ok(fresh)
}
