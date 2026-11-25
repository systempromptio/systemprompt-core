//! Overload shedding: a process-wide in-flight request ceiling.
//!
//! [`outermost`] is the last layer the server applies, so admission is
//! decided before CORS, sessions, authentication, rate limits and body
//! parsing. With `server.max_in_flight` set (the default is
//! `DEFAULT_MAX_IN_FLIGHT`), every request takes a permit for its whole
//! lifetime, from admission until the response body has been read to its end
//! or dropped, so an open SSE, A2A or gateway stream and a slow download each
//! count against the ceiling until they close; when none is free the request is
//! refused at once with `503`, `Retry-After: 1` and the `ApiError` envelope
//! (`error_key: "overloaded"`), instead of queueing until the balancer's
//! timeout. Health probes and `/metrics` never take a permit, so a saturated
//! replica still answers its orchestrator; `/readyz` reports `saturated` (read
//! from the request extension this middleware attaches) so the balancer steers
//! traffic away.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroU32;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::Held;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(crate) const HTTP_LOAD_SHED_TOTAL: &str = "http_load_shed_total";
pub(crate) const HTTP_IN_FLIGHT_LIMIT: &str = "http_in_flight_limit";
pub(crate) const HTTP_IN_FLIGHT_SATURATION: &str = "http_in_flight_saturation";

const EXEMPT_PATHS: [&str; 5] = [
    ApiPaths::LIVEZ,
    ApiPaths::READYZ,
    ApiPaths::HEALTH,
    ApiPaths::HEALTH_ROOT,
    "/metrics",
];

#[derive(Debug)]
pub(crate) struct LoadShed {
    permits: Arc<Semaphore>,
    max: u32,
}

impl LoadShed {
    pub(crate) fn new(max: NonZeroU32) -> Self {
        let max = max.get();
        metrics::gauge!(HTTP_IN_FLIGHT_LIMIT).set(f64::from(max));
        Self {
            permits: Arc::new(Semaphore::new(max as usize)),
            max,
        }
    }

    pub(crate) const fn limit(&self) -> u32 {
        self.max
    }

    pub(crate) fn in_flight(&self) -> u32 {
        let available = u32::try_from(self.permits.available_permits()).unwrap_or(self.max);
        self.max.saturating_sub(available)
    }

    pub(crate) fn saturated(&self) -> bool {
        self.permits.available_permits() == 0
    }

    pub(crate) fn saturation(&self) -> f64 {
        f64::from(self.in_flight()) / f64::from(self.max)
    }

    fn publish_saturation(&self) {
        metrics::gauge!(HTTP_IN_FLIGHT_SATURATION).set(self.saturation());
    }
}

pub fn outermost(router: axum::Router, max_in_flight: Option<NonZeroU32>) -> axum::Router {
    match max_in_flight {
        Some(max) => router.layer(axum::middleware::from_fn_with_state(
            Arc::new(LoadShed::new(max)),
            shed,
        )),
        None => router,
    }
}

pub(crate) async fn shed(
    State(shed): State<Arc<LoadShed>>,
    mut req: Request,
    next: Next,
) -> Response {
    req.extensions_mut().insert(Arc::clone(&shed));
    if EXEMPT_PATHS.contains(&req.uri().path()) {
        return next.run(req).await;
    }

    let Ok(permit) = Arc::clone(&shed.permits).try_acquire_owned() else {
        metrics::counter!(HTTP_LOAD_SHED_TOTAL).increment(1);
        shed.publish_saturation();
        return overloaded();
    };
    shed.publish_saturation();
    let admitted = Admitted {
        permit: Some(permit),
        shed: Arc::clone(&shed),
    };
    next.run(req)
        .await
        .map(|body| Body::new(Held::new(body, admitted)))
}

#[derive(Debug)]
struct Admitted {
    permit: Option<OwnedSemaphorePermit>,
    shed: Arc<LoadShed>,
}

impl Drop for Admitted {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.shed.publish_saturation();
    }
}

fn overloaded() -> Response {
    let body = ApiError::service_unavailable("server at capacity").with_error_key("overloaded");
    let mut response = (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    response
}
