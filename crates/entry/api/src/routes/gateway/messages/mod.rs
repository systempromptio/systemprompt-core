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
    match handle_inner(
        Arc::clone(&inbound),
        &jwt_extractor,
        &ctx,
        request,
        &ai_request_id,
        &mut partial,
    )
    .await
    {
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

async fn handle_inner(
    inbound: Arc<dyn InboundAdapter>,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
    request: Request<Body>,
    ai_request_id: &AiRequestId,
    partial: &mut RejectionPartial,
) -> Result<Response<Body>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;
    let request_ctx = RequestContext {
        jwt_extractor,
        ctx,
        profile,
        ai_request_id,
    };
    let prepared = extract_request_context(&request_ctx, &inbound, request, partial).await?;
    dispatch_to_provider(&request_ctx, inbound, prepared).await
}
