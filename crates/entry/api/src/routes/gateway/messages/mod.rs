mod auth;
mod dispatch;
mod extract;
mod rejection;

pub use extract::extract_credential;

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

use dispatch::{build_error_response, dispatch_to_provider};
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
        Err((status, message)) => {
            tracing::warn!(
                status = %status,
                message = %message,
                ai_request_id = %ai_request_id,
                wire = inbound.wire_name(),
                "Gateway request rejected",
            );
            persist_rejection(&ctx, &ai_request_id, &partial, status, &message).await;
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
    async fn run(self, request: Request<Body>) -> Result<Response<Body>, (StatusCode, String)> {
        let profile = ProfileBootstrap::get().map_err(|e| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("Profile not ready: {e}"),
            )
        })?;
        let request_ctx = RequestContext {
            jwt_extractor: self.jwt_extractor,
            ctx: self.ctx,
            profile,
            ai_request_id: self.ai_request_id,
        };
        let prepared =
            extract_request_context(&request_ctx, &self.inbound, request, self.partial).await?;
        dispatch_to_provider(&request_ctx, self.inbound, prepared).await
    }
}
