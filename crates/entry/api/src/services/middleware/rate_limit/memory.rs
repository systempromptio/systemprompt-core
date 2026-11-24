//! [`MemoryRateLimitBuckets`]: the per-replica windows the global limiter
//! charges when the users member provides no replica-shared ones.
//!
//! It counts the same per-user, per-scope windows in process memory and
//! forgets every window older than the newest one it has been charged with,
//! so it holds at most the callers active in one window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
use systemprompt_traits::{RateLimitBuckets, RepositoryError};

#[derive(Debug, Default)]
pub struct MemoryRateLimitBuckets {
    windows: Mutex<Windows>,
}

#[derive(Debug, Default)]
struct Windows {
    newest: Option<DateTime<Utc>>,
    hits: HashMap<(UserId, String, DateTime<Utc>), i64>,
}

#[async_trait]
impl RateLimitBuckets for MemoryRateLimitBuckets {
    async fn hit(
        &self,
        user_id: &UserId,
        scope: &str,
        window_start: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.newest.is_none_or(|newest| window_start > newest) {
            windows.newest = Some(window_start);
            windows
                .hits
                .retain(|(_, _, start), _| *start >= window_start);
        }
        let hits = windows
            .hits
            .entry((user_id.clone(), scope.to_owned(), window_start))
            .or_insert(0);
        *hits = hits.saturating_add(1);
        let count = *hits;
        drop(windows);
        Ok(count)
    }
}
