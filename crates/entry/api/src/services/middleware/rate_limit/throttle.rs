//! The middleware that charges a [`KeyedLimiter`] per request, and
//! [`SubjectThrottle`], the same limiter for a handler that can name its
//! caller only after verifying a credential the request carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::router::ChargeRefused;
use systemprompt_traits::OwnedTask;

use super::key::{IdentityOrTrustedIpKey, TrustedIpKey};
use super::keyed::{KeyedLimiter, MAX_TRACKED_KEYS, SWEEP_INTERVAL};

#[derive(Debug, Clone)]
pub(super) enum Keying {
    IdentityOrIp(IdentityOrTrustedIpKey),
    Ip(TrustedIpKey),
}

impl Keying {
    fn key(&self, req: &Request) -> Option<String> {
        match self {
            Self::IdentityOrIp(key) => key.key(req),
            Self::Ip(key) => key.key(req),
        }
    }
}

#[derive(Debug)]
pub(super) struct Throttle {
    limiter: Arc<KeyedLimiter>,
    keying: Keying,
    _sweeper: OwnedTask<()>,
}

impl Throttle {
    pub(super) fn new(emission: Duration, burst: u32, keying: Keying) -> Arc<Self> {
        let limiter = Arc::new(KeyedLimiter::new(emission, burst, MAX_TRACKED_KEYS));
        let sweeper = limiter.spawn_sweeper(SWEEP_INTERVAL);
        Arc::new(Self {
            limiter,
            keying,
            _sweeper: sweeper,
        })
    }
}

/// A per-replica limiter charged by a key the handler derives from a verified
/// credential.
#[derive(Debug)]
pub struct SubjectThrottle {
    limiter: Arc<KeyedLimiter>,
    _sweeper: OwnedTask<()>,
}

impl SubjectThrottle {
    pub(super) fn new(emission: Duration, burst: u32) -> Arc<Self> {
        let limiter = Arc::new(KeyedLimiter::new(emission, burst, MAX_TRACKED_KEYS));
        let sweeper = limiter.spawn_sweeper(SWEEP_INTERVAL);
        Arc::new(Self {
            limiter,
            _sweeper: sweeper,
        })
    }

    pub fn charge(&self, key: &str) -> Result<(), Throttled> {
        self.limiter.check(key, Instant::now()).map_err(Throttled)
    }
}

/// A refused [`SubjectThrottle::charge`]: answers 429 with `Retry-After`.
#[derive(Debug, Clone, Copy)]
pub struct Throttled(Duration);

impl systemprompt_extension::router::SubjectCharge for SubjectThrottle {
    fn charge(&self, key: &str) -> Result<(), ChargeRefused> {
        Self::charge(self, key).map_err(|Throttled(wait)| ChargeRefused { retry_after: wait })
    }
}

impl IntoResponse for Throttled {
    fn into_response(self) -> Response {
        ChargeRefused {
            retry_after: self.0,
        }
        .into_response()
    }
}

pub(super) async fn throttle(
    State(throttle): State<Arc<Throttle>>,
    req: Request,
    next: Next,
) -> Response {
    let Some(key) = throttle.keying.key(&req) else {
        tracing::error!("rate limiter could not resolve a client key");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    match throttle.limiter.check(&key, Instant::now()) {
        Ok(()) => next.run(req).await,
        Err(wait) => ChargeRefused { retry_after: wait }.into_response(),
    }
}
