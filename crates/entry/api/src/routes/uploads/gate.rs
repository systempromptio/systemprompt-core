//! Signed upload tokens: a request carrying one, in the `x-upload-token`
//! header or (for a client that cannot set headers) the `access_token`
//! query, is answered here, before the session middleware. The token must
//! verify for the upload audience and its `purpose` must name the upload in
//! the path. A write token opens only that upload's part PUTs, completion
//! and progress; a download token only its `GET`/`HEAD /{id}/content`. The
//! request is then routed, unauthenticated by session, as the token's
//! subject, whose account must still be active. Every other request passes
//! through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::{DocumentUploadId, TraceId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::JwtAudience;
use systemprompt_runtime::documents::download::DOWNLOAD_TOKEN_QUERY;
use systemprompt_runtime::documents::upload_links::{
    UPLOAD_AUDIENCE, UPLOAD_TOKEN_HEADER, UploadGrant, upload_purpose,
};
use tower::ServiceExt;

use super::principal::{TokenUploader, Uploader};
use crate::error::ApiHttpError;
use crate::routes::documents::DocumentsState;

#[derive(Clone)]
pub(super) struct GateState {
    pub(super) documents: DocumentsState,
    pub(super) tokened: Router,
}

fn token(request: &Request) -> Option<String> {
    let header = request
        .headers()
        .get(UPLOAD_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    header.or_else(|| {
        let query = request.uri().query()?;
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == DOWNLOAD_TOKEN_QUERY)
            .map(|(_, value)| value.into_owned())
    })
}

/// Whether a write token may make this call: only the upload's own part
/// PUTs, its completion and its progress.
fn writable(method: &Method, rest: &str) -> bool {
    match (method, rest.split_once('/')) {
        (&Method::PUT, Some(("parts", part))) => !part.is_empty() && !part.contains('/'),
        (&Method::POST, None) => rest == "complete",
        (&Method::GET | &Method::HEAD, None) => rest.is_empty(),
        _ => false,
    }
}

pub(super) async fn gate(
    State(state): State<GateState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = token(&request) else {
        return next.run(request).await;
    };
    match authorize(&state.documents, &token, &request).await {
        Ok(uploader) => {
            request.extensions_mut().insert(TokenUploader(uploader));
            match state.tokened.oneshot(request).await {
                Ok(response) => response,
                Err(never) => match never {},
            }
        },
        Err(error) => error.into_response(),
    }
}

async fn authorize(
    state: &DocumentsState,
    token: &str,
    request: &Request,
) -> Result<Uploader, ApiHttpError> {
    let audience = [JwtAudience::Resource(UPLOAD_AUDIENCE.to_owned())];
    let claims = state
        .tokens
        .verify_bearer(token, &audience)
        .map_err(|source| {
            ApiError::unauthorized("upload link is invalid or has expired").with_source(source)
        })?;
    let path = request.uri().path().trim_matches('/');
    let (id, rest) = path.split_once('/').unwrap_or((path, ""));
    let id = DocumentUploadId::try_new(id)?;
    let purpose = claims.custom.purpose.as_deref();
    let method = request.method();
    let readable = matches!(*method, Method::GET | Method::HEAD) && rest == "content";
    let allowed = (writable(method, rest)
        && purpose == Some(upload_purpose(UploadGrant::Write, &id).as_str()))
        || (readable && purpose == Some(upload_purpose(UploadGrant::Download, &id).as_str()));
    if !allowed {
        return Err(ApiHttpError::forbidden(
            "upload token is not for this upload or action",
        ));
    }
    let user = UserId::try_new(claims.registered.sub)?;
    match state.users.find_by_id(&user).await {
        Ok(Some(account)) if account.is_active => Ok(Uploader {
            user,
            trace: TraceId::generate(),
        }),
        Ok(_) => Err(ApiHttpError::unauthorized("account is not active")),
        Err(source) => Err(ApiError::internal("upload caller lookup failed", source).into()),
    }
}
