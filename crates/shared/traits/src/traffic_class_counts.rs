//! Cross-domain read of the session traffic-class counter the users domain
//! maintains at write time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::RepositoryError;

/// Sessions per traffic class over a window, summed from hourly buckets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrafficClassTotals {
    pub human: i64,
    pub ghost: i64,
    pub bot: i64,
}

/// Owned by the users domain and injected into analytics as `dyn`, hence
/// `#[async_trait]`.
///
/// The window is read in whole hour buckets: `start` is truncated down to
/// its hour, and every bucket before `end` counts.
#[async_trait]
pub trait TrafficClassCounts: Send + Sync + std::fmt::Debug {
    async fn session_totals(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TrafficClassTotals, RepositoryError>;
}

pub type DynTrafficClassCounts = std::sync::Arc<dyn TrafficClassCounts>;
