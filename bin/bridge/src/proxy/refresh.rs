//! The background task that keeps a served proxy's cached token fresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::token_cache::TokenCache;
use super::{REFRESH_THRESHOLD_SECS, REFRESH_TICK};

pub(super) async fn refresh_loop(cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(e) = cache.refresh_if_cached(REFRESH_THRESHOLD_SECS).await {
            tracing::debug!(error = %e, "token refresh tick did not renew");
        }
    }
}
