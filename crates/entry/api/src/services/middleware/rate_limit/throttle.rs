//! The middleware that charges a [`KeyedLimiter`] per request, and
//! [`SubjectThrottle`], the same limiter for a handler that can name its
//! caller only after verifying a credential the request carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
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

impl IntoResponse for Throttled {
    fn into_response(self) -> Response {
        too_many_requests(self.0)
    }
}

fn too_many_requests(wait: Duration) -> Response {
    let secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    let mut resp = StatusCode::TOO_MANY_REQUESTS.into_response();
    if let Ok(value) = HeaderValue::from_str(&secs.max(1).to_string()) {
        resp.headers_mut().insert(header::RETRY_AFTER, value);
    }
    resp
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
        Err(wait) => too_many_requests(wait),
    }
}
