//! The headers every response carrying user-uploaded bytes is served with.
//!
//! `nosniff` stops a browser from second-guessing the declared type, and a
//! `sandbox` CSP with `default-src 'none'` makes a body that does render
//! (HTML, SVG, a PDF) an opaque-origin document that runs no script and
//! loads nothing. Downloads are attachments as well; these headers hold
//! even when a browser shows one anyway.
//!
//! The response also carries a [`FrameOptionsOverride`] (same origin): the
//! security-headers middleware replaces a route's CSP with the app's own
//! unless that override is present, in which case it keeps the route's CSP
//! and only sets `frame-ancestors`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderValue, header};
use axum::response::Response;
use systemprompt_extension::FrameOptionsOverride;
use systemprompt_provider_contracts::FrameOptions;

pub(crate) const USER_CONTENT_CSP: &str = "sandbox; default-src 'none'";

/// The CSP of a PDF shown inline in the app: sandboxed, and frameable only
/// by the app itself.
pub(crate) const INLINE_PREVIEW_CSP: &str = "sandbox; default-src 'none'; frame-ancestors 'self'";

pub(crate) fn harden(response: &mut Response, csp: &'static str) {
    if response
        .extensions()
        .get::<FrameOptionsOverride>()
        .is_none()
    {
        response
            .extensions_mut()
            .insert(FrameOptionsOverride(FrameOptions::SameOrigin));
    }
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(csp),
    );
}
