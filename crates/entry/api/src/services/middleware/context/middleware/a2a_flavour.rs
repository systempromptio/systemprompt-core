//! The A2A context flavour.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_security::HeaderExtractor;
use tracing::Instrument;

use super::{ContextExtractor, DynExtractor, create_request_span, log_error_response};
use crate::services::middleware::body::BodyBudget;

const BUDGET_RETRY_AFTER_SECS: &str = "1";

/// A2A flavour: requires a real user.
///
/// Parses the JSON-RPC body to recover `contextId` (the A2A wire spec carries
/// it in the body, not headers), so the body is buffered, up to the router's
/// body limit, and rebuilt for the handler. Every request first reserves its
/// declared length (or the full limit when it declares none) from the
/// router's [`BodyBudget`] and holds it until its response is produced, so the
/// bytes this router buffers at once never exceed the budget; a request that
/// cannot reserve answers `503` with `Retry-After`.
#[derive(Clone)]
pub struct A2AContextMiddleware {
    extractor: DynExtractor,
    body_limit: usize,
    budget: BodyBudget,
}

impl std::fmt::Debug for A2AContextMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2AContextMiddleware").finish()
    }
}

impl A2AContextMiddleware {
    pub fn new<E>(extractor: E, body_limit: usize, budget: BodyBudget) -> Self
    where
        E: ContextExtractor + Send + Sync + 'static,
    {
        Self {
            extractor: Arc::new(extractor),
            body_limit,
            budget,
        }
    }

    pub async fn handle(&self, request: Request, next: Next) -> Response {
        let Some(_reservation) = self.budget.reserve(request.headers(), self.body_limit) else {
            tracing::warn!("a2a body budget exhausted; refusing the request");
            let mut resp = StatusCode::SERVICE_UNAVAILABLE.into_response();
            resp.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_static(BUDGET_RETRY_AFTER_SECS),
            );
            return resp;
        };
        let trace_id = HeaderExtractor::extract_trace_id(request.headers());
        let path = request.uri().path().to_owned();
        let method = request.method().to_string();

        match self
            .extractor
            .extract_from_request(request, self.body_limit)
            .await
        {
            Ok((context, reconstructed_request)) => {
                let span = create_request_span(&context);
                let mut req = reconstructed_request;
                req.extensions_mut().insert(context);
                next.run(req).instrument(span).await
            },
            Err(e) => log_error_response(&e, &trace_id, &path, &method),
        }
    }
}
