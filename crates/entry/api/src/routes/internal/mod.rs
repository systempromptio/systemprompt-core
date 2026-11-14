//! `/api/v1/internal/*`: calls only a managed MCP server makes.
//!
//! `POST /api/v1/internal/links` signs a document download or upload link for
//! the user of the MCP call being served. The route takes no session: the
//! caller presents the proxy's hop token for that call in `x-mcp-hop`, and
//! [`verify_link_hop`] verifies it for the server the body names;
//! [`mint_verified`] takes the subject from its claims and re-checks the
//! subject's access to the target before signing. A missing, expired or foreign
//! hop token answers 401; a server that hosts no `documents` toolset (an upload
//! link also admits `issues`), or a subject who may not open the target,
//! answers 403.
//!
//! The route is throttled per verified subject: the hop is verified first,
//! then the server-and-user key is charged, so the managed servers, which
//! all call from loopback, never share one budget. Unauthenticated requests
//! are bounded by the server-wide pre-auth address throttle and are logged
//! at debug only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use http::HeaderMap;
use systemprompt_identifiers::headers;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_runtime::documents::links::{
    LinkMintError, LinkMinter, LinkRequest, mint_verified, verify_link_hop,
};

use crate::kernel::{ApiHttpError, SubjectThrottle};

#[derive(Debug)]
struct LinksState {
    minter: LinkMinter,
    throttle: Option<Arc<SubjectThrottle>>,
}

pub fn links_router(minter: LinkMinter, throttle: Option<Arc<SubjectThrottle>>) -> Router {
    Router::new()
        .route("/", post(issue_link))
        .with_state(Arc::new(LinksState { minter, throttle }))
}

async fn issue_link(
    State(state): State<Arc<LinksState>>,
    request_headers: HeaderMap,
    Json(request): Json<LinkRequest>,
) -> Response {
    let Some(hop) = request_headers
        .get(headers::MCP_HOP)
        .and_then(|value| value.to_str().ok())
    else {
        return ApiHttpError::from(ApiError::new(
            ErrorCode::Unauthorized,
            "a link request must carry the hop token of the MCP call it serves",
        ))
        .into_response();
    };
    let claims = match verify_link_hop(&state.minter, hop, &request) {
        Ok(claims) => claims,
        Err(error) => {
            tracing::debug!(server = %request.server, error = %error, cause = ?std::error::Error::source(&error), "refused an unverified internal link request");
            return ApiHttpError::from(refusal(error)).into_response();
        },
    };
    if let Some(throttle) = &state.throttle
        && let Err(throttled) =
            throttle.charge(&format!("{}:{}", request.server, claims.identity.user_id))
    {
        return throttled.into_response();
    }
    match mint_verified(&state.minter, &claims, &request).await {
        Ok(link) => Json(link).into_response(),
        Err(error) => {
            tracing::warn!(server = %request.server, error = %error, cause = ?std::error::Error::source(&error), "refused an internal link request");
            ApiHttpError::from(refusal(error)).into_response()
        },
    }
}

fn refusal(error: LinkMintError) -> ApiError {
    let code = match &error {
        LinkMintError::Hop(_) => ErrorCode::Unauthorized,
        LinkMintError::NotHosted { .. }
        | LinkMintError::Anonymous
        | LinkMintError::Upload(_)
        | LinkMintError::Document(_) => ErrorCode::Forbidden,
        LinkMintError::UploadClosed { .. } => ErrorCode::ConflictError,
        LinkMintError::Signing(_) => {
            return ApiError::internal("the link could not be signed", error);
        },
    };
    ApiError::new(code, error.to_string())
}
