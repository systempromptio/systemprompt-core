//! Page framing policy shared by the server profile and extension routers.
//!
//! [`FrameOptions`] is the profile's sitewide `X-Frame-Options` default and
//! the per-router override an extension declares; its serialised form is the
//! header token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameOptions {
    #[serde(rename = "DENY")]
    Deny,
    #[serde(rename = "SAMEORIGIN")]
    SameOrigin,
    #[serde(rename = "ALLOWALL")]
    AllowAll,
}

impl FrameOptions {
    #[must_use]
    pub const fn header_value(self) -> Option<&'static str> {
        match self {
            Self::Deny => Some("DENY"),
            Self::SameOrigin => Some("SAMEORIGIN"),
            Self::AllowAll => None,
        }
    }

    #[must_use]
    pub const fn frame_ancestors(self) -> &'static str {
        match self {
            Self::Deny => "'none'",
            Self::SameOrigin => "'self'",
            Self::AllowAll => "*",
        }
    }
}
