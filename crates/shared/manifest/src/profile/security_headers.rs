//! The response security headers a server sends, and the closed
//! `Referrer-Policy` directive set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_provider_contracts::FrameOptions;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecurityHeadersConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_hsts")]
    pub hsts: String,

    #[serde(default = "default_frame_options")]
    #[schemars(with = "String")]
    pub frame_options: FrameOptions,

    #[serde(default = "default_content_type_options")]
    pub content_type_options: String,

    #[serde(default)]
    pub referrer_policy: ReferrerPolicy,

    #[serde(default = "default_permissions_policy")]
    pub permissions_policy: String,

    #[serde(default)]
    pub content_security_policy: Option<String>,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hsts: default_hsts(),
            frame_options: default_frame_options(),
            content_type_options: default_content_type_options(),
            referrer_policy: ReferrerPolicy::default(),
            permissions_policy: default_permissions_policy(),
            content_security_policy: None,
        }
    }
}

const fn default_enabled() -> bool {
    true
}

fn default_hsts() -> String {
    "max-age=63072000; includeSubDomains; preload".to_owned()
}

const fn default_frame_options() -> FrameOptions {
    FrameOptions::Deny
}

fn default_content_type_options() -> String {
    "nosniff".to_owned()
}

fn default_permissions_policy() -> String {
    "camera=(), microphone=(), geolocation=()".to_owned()
}

/// `Referrer-Policy` directive. A closed set — an unknown value in the
/// profile is a load error rather than a header the browser silently ignores.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum ReferrerPolicy {
    #[serde(rename = "no-referrer")]
    NoReferrer,
    #[serde(rename = "no-referrer-when-downgrade")]
    NoReferrerWhenDowngrade,
    #[serde(rename = "origin")]
    Origin,
    #[serde(rename = "origin-when-cross-origin")]
    OriginWhenCrossOrigin,
    #[serde(rename = "same-origin")]
    SameOrigin,
    #[serde(rename = "strict-origin")]
    StrictOrigin,
    #[default]
    #[serde(rename = "strict-origin-when-cross-origin")]
    StrictOriginWhenCrossOrigin,
    #[serde(rename = "unsafe-url")]
    UnsafeUrl,
}

impl ReferrerPolicy {
    #[must_use]
    pub const fn header_value(self) -> &'static str {
        match self {
            Self::NoReferrer => "no-referrer",
            Self::NoReferrerWhenDowngrade => "no-referrer-when-downgrade",
            Self::Origin => "origin",
            Self::OriginWhenCrossOrigin => "origin-when-cross-origin",
            Self::SameOrigin => "same-origin",
            Self::StrictOrigin => "strict-origin",
            Self::StrictOriginWhenCrossOrigin => "strict-origin-when-cross-origin",
            Self::UnsafeUrl => "unsafe-url",
        }
    }
}
