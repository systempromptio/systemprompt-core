//! Gateway message-dispatch entry point.
//!
//! [`handle`] is the shared handler behind the `/messages` and `/responses`
//! routes: it builds a `RequestContext`, extracts and authorizes the request
//! (`extract`), then dispatches to the resolved provider (`dispatch`),
//! persisting a rejection record (`rejection`) on any early failure. Inbound
//! wire format is selected by the [`InboundAdapter`] passed in by the router.

mod auth;
mod dispatch;
mod extract;
mod rejection;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::auth::{ApiKeyPrincipal, AuthedPrincipal, JwtPrincipal};
}

pub use dispatch::map_upstream_error;
pub use extract::{GatewayAuthzRequestInput, build_gateway_authz_request, extract_credential};

use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::Response;
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::Profile;
use systemprompt_runtime::AppContext;

use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::middleware::JwtContextExtractor;

use dispatch::{RejectionError, build_error_response, dispatch_to_provider};
use extract::{RejectionPartial, extract_request_context};
use rejection::persist_rejection;

pub(super) struct RequestContext<'a> {
    pub jwt_extractor: &'a JwtContextExtractor,
    pub ctx: &'a AppContext,
    pub profile: &'a Profile,
    pub ai_request_id: &'a AiRequestId,
}

pub async fn handle(
    inbound: Arc<dyn InboundAdapter>,
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    request: Request<Body>,
) -> Response<Body> {
    let ai_request_id = AiRequestId::generate();
    let mut partial = RejectionPartial::default();
    let inner = HandleInner {
        inbound: Arc::clone(&inbound),
        jwt_extractor: &jwt_extractor,
        ctx: &ctx,
        ai_request_id: &ai_request_id,
        partial: &mut partial,
    };
    match inner.run(request).await {
        Ok(resp) => resp,
        Err(RejectionError {
            status,
            message,
            persist,
        }) => {
            tracing::warn!(
                status = %status,
                message = %message,
                ai_request_id = %ai_request_id,
                wire = inbound.wire_name(),
                "Gateway request rejected",
            );
            if persist {
                persist_rejection(&ctx, &ai_request_id, &partial, status, &message).await;
            }
            let body = inbound.render_error(status, &message);
            Response::builder()
                .status(status)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap_or_else(|_| build_error_response(status, &message))
        },
    }
}

struct HandleInner<'a> {
    inbound: Arc<dyn InboundAdapter>,
    jwt_extractor: &'a JwtContextExtractor,
    ctx: &'a AppContext,
    ai_request_id: &'a AiRequestId,
    partial: &'a mut RejectionPartial,
}

impl HandleInner<'_> {
    async fn run(self, request: Request<Body>) -> Result<Response<Body>, RejectionError> {
        let profile = ProfileBootstrap::get().map_err(|e| RejectionError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: format!("Profile not ready: {e}"),
            persist: true,
        })?;
        let request_ctx = RequestContext {
            jwt_extractor: self.jwt_extractor,
            ctx: self.ctx,
            profile,
            ai_request_id: self.ai_request_id,
        };
        let prepared = extract_request_context(&request_ctx, &self.inbound, request, self.partial)
            .await
            .map_err(|(status, message)| RejectionError {
                status,
                message,
                persist: true,
            })?;
        dispatch_to_provider(&request_ctx, self.inbound, prepared).await
    }
}
