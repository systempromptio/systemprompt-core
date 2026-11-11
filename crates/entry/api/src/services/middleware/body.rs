//! The one bounded request-body reader.
//!
//! `DefaultBodyLimit` is an extension only the `Json`/`Bytes` extractors
//! consult; a handler or middleware that reads a raw [`Body`] bypasses it.
//! Every such read goes through [`read_body_bounded`], which refuses a body
//! whose declared length (its size hint, or for a whole request its
//! `Content-Length`) already exceeds the limit before reading a byte, and
//! stops reading as soon as a streamed body crosses it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::{Body, HttpBody};
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::http::header::CONTENT_LENGTH;
use axum::http::request::Parts;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug, thiserror::Error)]
pub enum BodyReadError {
    #[error("request body exceeds {limit} bytes")]
    TooLarge { limit: usize },
    #[error("request body could not be read")]
    Unreadable(#[source] axum::Error),
}

impl BodyReadError {
    #[must_use]
    pub const fn is_too_large(&self) -> bool {
        matches!(self, Self::TooLarge { .. })
    }
}

pub async fn read_body_bounded(body: Body, limit: usize) -> Result<Bytes, BodyReadError> {
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if body.size_hint().lower() > limit_u64 {
        return Err(BodyReadError::TooLarge { limit });
    }
    axum::body::to_bytes(body, limit).await.map_err(|error| {
        let oversized = std::error::Error::source(&error)
            .is_some_and(<dyn std::error::Error>::is::<http_body_util::LengthLimitError>);
        if oversized {
            BodyReadError::TooLarge { limit }
        } else {
            BodyReadError::Unreadable(error)
        }
    })
}

pub async fn read_request_bounded(
    request: Request<Body>,
    limit: usize,
) -> Result<(Parts, Bytes), BodyReadError> {
    let (parts, body) = request.into_parts();
    let declared = parts
        .headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    if declared.is_some_and(|length| length > u64::try_from(limit).unwrap_or(u64::MAX)) {
        return Err(BodyReadError::TooLarge { limit });
    }
    let bytes = read_body_bounded(body, limit).await?;
    Ok((parts, bytes))
}

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
