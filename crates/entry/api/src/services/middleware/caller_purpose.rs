//! Strips a client-sent caller purpose at the API edge.
//!
//! The caller purpose (`x-systemprompt-caller`) tells the credential broker
//! which workflow is asking for a credential, and the broker may refuse by
//! it. It is set by the issuer of a credential (an API key's `purpose`, a
//! JWT's `purpose` claim), never by its bearer, so an inbound copy of the
//! header is removed before any handler, proxy or gateway route sees it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_identifiers::headers;

pub async fn strip_caller_purpose(mut request: Request, next: Next) -> Response {
    if request
        .headers_mut()
        .remove(headers::CALLER_PURPOSE)
        .is_some()
    {
        tracing::debug!("dropped a client-sent caller purpose header");
    }
    next.run(request).await
}
