//! The replica-shared per-user request windows the API's global rate limit
//! charges.
//!
//! The window table belongs to the users member, which provides a
//! [`DynRateLimitBuckets`]; the API's global limiter only charges it. The
//! view takes one hit per request and reads back the window's count: it cannot
//! reset, read without charging, or prune a window, so a holder can spend a
//! caller's budget but never restore it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use crate::RepositoryError;

/// Charges one hit to a user's window for a scope and returns the window's
/// count after the hit. `#[async_trait]` keeps the trait `dyn`-compatible
/// for [`DynRateLimitBuckets`].
#[async_trait]
pub trait RateLimitBuckets: Send + Sync + Debug {
    async fn hit(
        &self,
        user_id: &UserId,
        scope: &str,
        window_start: DateTime<Utc>,
    ) -> Result<i64, RepositoryError>;
}

pub type DynRateLimitBuckets = Arc<dyn RateLimitBuckets>;
