//! Per-route framing policy override for extension routers.
//!
//! The host's global security-headers middleware sets `X-Frame-Options`
//! sitewide. An extension that must allow its pages to be framed (embed
//! widgets, chrome-free tool pages) declares a [`FrameOptions`] on its
//! router; [`stamp_frame_options`] records the choice as a
//! [`FrameOptionsOverride`] response extension, which the host middleware
//! honours instead of the profile default. Setting the raw header without
//! the marker has no effect — the global middleware overwrites it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOptions {
    Deny,
    SameOrigin,
    AllowAll,
}

impl FrameOptions {
    /// `X-Frame-Options` value for this policy. `None` for [`Self::AllowAll`]:
    /// the header has no allow-all value, so absence is the mechanism.
    #[must_use]
    pub const fn header_value(self) -> Option<&'static str> {
        match self {
            Self::Deny => Some("DENY"),
            Self::SameOrigin => Some("SAMEORIGIN"),
            Self::AllowAll => None,
        }
    }

    /// Source list for the equivalent `Content-Security-Policy:
    /// frame-ancestors` directive.
    #[must_use]
    pub const fn frame_ancestors(self) -> &'static str {
        match self {
            Self::Deny => "'none'",
            Self::SameOrigin => "'self'",
            Self::AllowAll => "*",
        }
    }
}

/// Response-extension marker read by the host's security-headers middleware.
#[derive(Debug, Clone, Copy)]
pub struct FrameOptionsOverride(pub FrameOptions);

pub async fn stamp_frame_options(
    frame_options: FrameOptions,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    response
        .extensions_mut()
        .insert(FrameOptionsOverride(frame_options));
    response
}
