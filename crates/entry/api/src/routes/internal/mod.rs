//! `/api/v1/internal/*`: calls only a managed MCP server makes.
//!
//! `POST /api/v1/internal/links` signs a document download or upload link for
//! the user of the MCP call being served. The route takes no session: the
//! caller presents the proxy's hop token for that call in `x-mcp-hop`, and
//! [`mint_link`] verifies it for the server the body names, takes the subject
//! from its claims and re-checks the subject's access to the target before
//! signing. A missing, expired or foreign hop token answers 401; a subject
//! who may not open the target answers 403.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use http::HeaderMap;
use systemprompt_identifiers::headers;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_runtime::documents::links::{
    IssuedLink, LinkMintError, LinkMinter, LinkRequest, mint_link,
};

use crate::error::ApiHttpError;

pub fn links_router(minter: LinkMinter) -> Router {
    Router::new()
        .route("/", post(issue_link))
        .with_state(Arc::new(minter))
}

async fn issue_link(
    State(minter): State<Arc<LinkMinter>>,
    request_headers: HeaderMap,
    Json(request): Json<LinkRequest>,
) -> Result<Json<IssuedLink>, ApiHttpError> {
    let hop = request_headers
        .get(headers::MCP_HOP)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unauthorized,
                "a link request must carry the hop token of the MCP call it serves",
            )
        })?;
    match mint_link(&minter, hop, &request).await {
        Ok(link) => Ok(Json(link)),
        Err(error) => {
            tracing::warn!(server = %request.server, error = %error, cause = ?std::error::Error::source(&error), "refused an internal link request");
            Err(ApiHttpError::from(refusal(error)))
        },
    }
}

fn refusal(error: LinkMintError) -> ApiError {
    let code = match &error {
        LinkMintError::Hop(_) => ErrorCode::Unauthorized,
        LinkMintError::Anonymous | LinkMintError::Upload(_) | LinkMintError::Document(_) => {
            ErrorCode::Forbidden
        },
        LinkMintError::UploadClosed { .. } => ErrorCode::ConflictError,
        LinkMintError::Signing(_) => {
            return ApiError::internal("the link could not be signed", error);
        },
    };
    ApiError::new(code, error.to_string())
}
