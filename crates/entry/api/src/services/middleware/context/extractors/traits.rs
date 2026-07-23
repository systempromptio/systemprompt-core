//! `ContextExtractor` contract for header/body context extraction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderMap;
use systemprompt_models::execution::{ContextExtractionError, RequestContext};

// Why: `#[async_trait]`: ContextExtractor is dispatched as a trait object (`dyn
// _`), so it must be `dyn`-compatible; native `async fn` in traits is not.
#[async_trait]
pub trait ContextExtractor: Send + Sync {
    async fn extract_from_headers(
        &self,
        headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError>;

    async fn extract_from_request(
        &self,
        request: Request<Body>,
    ) -> Result<(RequestContext, Request<Body>), ContextExtractionError> {
        let headers = request.headers().clone();
        let context = self.extract_from_headers(&headers).await?;
        Ok((context, request))
    }
}
