//! The router-wide byte budget for buffered request bodies ([`BodyBudget`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use axum::http::header::CONTENT_LENGTH;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const BUDGET_UNIT_BYTES: usize = 1024 * 1024;

/// A byte budget shared by every buffered read on one router.
///
/// Each request reserves its declared length, rounded up to whole MiB, or the
/// router's full body limit when it declares none, and holds the reservation
/// until its response is produced. The bytes buffered at once on that router
/// therefore never exceed the budget; a request that cannot reserve is refused
/// at once rather than queued.
#[derive(Debug, Clone)]
pub struct BodyBudget {
    permits: Arc<Semaphore>,
    total_units: u32,
}

#[derive(Debug)]
pub struct BodyReservation(#[expect(dead_code, reason = "held for its Drop")] OwnedSemaphorePermit);

impl BodyBudget {
    #[must_use]
    pub fn new(budget_bytes: usize) -> Self {
        let total_units = u32::try_from(budget_bytes.div_ceil(BUDGET_UNIT_BYTES))
            .unwrap_or(u32::MAX)
            .max(1);
        Self {
            permits: Arc::new(Semaphore::new(total_units as usize)),
            total_units,
        }
    }

    #[must_use]
    pub fn available_bytes(&self) -> usize {
        self.permits.available_permits() * BUDGET_UNIT_BYTES
    }

    pub fn reserve(&self, headers: &HeaderMap, body_limit: usize) -> Option<BodyReservation> {
        let declared = headers
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<usize>().ok())
            .map_or(body_limit, |length| length.min(body_limit));
        let units = u32::try_from(declared.div_ceil(BUDGET_UNIT_BYTES))
            .unwrap_or(u32::MAX)
            .clamp(1, self.total_units);
        Arc::clone(&self.permits)
            .try_acquire_many_owned(units)
            .ok()
            .map(BodyReservation)
    }
}
