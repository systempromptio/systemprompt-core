//! Server configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use ipnet::IpNet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use systemprompt_extension::FrameOptions;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub host: String,

    pub port: u16,

    pub api_server_url: String,

    pub api_internal_url: String,

    pub api_external_url: String,

    #[serde(default)]
    pub use_https: bool,

    #[serde(default)]
    pub cors_allowed_origins: Vec<String>,

    #[serde(default)]
    pub content_negotiation: ContentNegotiationConfig,

    #[serde(default)]
    pub security_headers: SecurityHeadersConfig,

    /// Stable identifier for this replica. Empty/unset resolves to the
    /// OS hostname (or a generated short id) at config build time.
    #[serde(default)]
    pub instance_id: Option<String>,

    /// Global cap on concurrent A2A SSE streams for this replica.
    #[serde(default = "default_max_concurrent_streams")]
    pub max_concurrent_streams: usize,

    /// CIDR ranges whose immediate-peer requests are allowed to set
    /// `X-Forwarded-For`, `X-Real-IP`, and `CF-Connecting-IP`. Empty
    /// means the platform treats every connection as direct and ignores
    /// those headers — the only safe default behind no proxy. Each entry
    /// is a CIDR string (e.g. `10.0.0.0/8`, `192.168.1.0/24`,
    /// `2001:db8::/32`). Single addresses without a `/` are accepted as
    /// `/32` (IPv4) or `/128` (IPv6). Invalid entries fail profile load —
    /// a silently-dropped proxy would demote a trusted hop to hostile and
    /// break client-IP resolution with no boot-time signal.
    #[serde(
        default,
        deserialize_with = "deserialize_trusted_proxies",
        serialize_with = "serialize_trusted_proxies"
    )]
    #[schemars(with = "Vec<String>")]
    pub trusted_proxies: Vec<IpNet>,
}

fn parse_trusted_proxy(entry: &str) -> Result<IpNet, String> {
    let trimmed = entry.trim();
    if let Ok(net) = trimmed.parse::<IpNet>() {
        return Ok(net);
    }
    match trimmed.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => Ok(IpNet::from(ipnet::Ipv4Net::from(v4))),
        Ok(IpAddr::V6(v6)) => Ok(IpNet::from(ipnet::Ipv6Net::from(v6))),
        Err(_) => Err(format!(
            "'{trimmed}' is not a valid CIDR range or IP address"
        )),
    }
}

fn deserialize_trusted_proxies<'de, D>(deserializer: D) -> Result<Vec<IpNet>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<String>::deserialize(deserializer)?;
    raw.iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| parse_trusted_proxy(s).map_err(serde::de::Error::custom))
        .collect()
}

fn serialize_trusted_proxies<S>(nets: &[IpNet], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_seq(nets.iter().map(ToString::to_string))
}

const fn default_max_concurrent_streams() -> usize {
    crate::config::DEFAULT_MAX_CONCURRENT_STREAMS
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContentNegotiationConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_markdown_suffix")]
    pub markdown_suffix: String,
}

impl Default for ContentNegotiationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            markdown_suffix: default_markdown_suffix(),
        }
    }
}

fn default_markdown_suffix() -> String {
    ".md".to_owned()
}

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
