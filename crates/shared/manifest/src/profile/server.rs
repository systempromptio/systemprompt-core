//! Server configuration.
//!
//! `max_in_flight` is the API's admission ceiling: an omitted key is
//! [`DEFAULT_MAX_IN_FLIGHT`] in-flight requests per replica (the gateway's
//! default admission limit), an explicit `null` is no ceiling.
//! `max_probe_in_flight` is the separate, smaller ceiling the health,
//! readiness and metrics routes share ([`DEFAULT_MAX_PROBE_IN_FLIGHT`]), so a
//! flood of probes cannot exhaust the server and a saturated main ceiling
//! never fails liveness. `max_route_body_bytes` is the ceiling a route's own
//! byte limit may declare ([`DEFAULT_MAX_ROUTE_BODY_BYTES`]); zero and values
//! above [`MAX_ROUTE_BODY_BYTES_LIMIT`] are refused at parse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use ipnet::IpNet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_identifiers::InstanceId;

pub use systemprompt_provider_contracts::FrameOptions;

use super::security_headers::SecurityHeadersConfig;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub host: String,

    pub port: u16,

    pub external_url: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_cors_origins: Vec<String>,

    #[serde(skip)]
    pub api_server_url: String,

    #[serde(skip)]
    pub api_internal_url: String,

    #[serde(skip)]
    pub api_external_url: String,

    #[serde(skip)]
    pub use_https: bool,

    #[serde(skip)]
    pub cors_allowed_origins: Vec<String>,

    #[serde(default)]
    pub content_negotiation: ContentNegotiationConfig,

    #[serde(default)]
    pub security_headers: SecurityHeadersConfig,

    #[serde(default, deserialize_with = "deserialize_instance_id")]
    pub instance_id: Option<InstanceId>,

    #[serde(default)]
    pub metrics_port: Option<u16>,

    #[serde(default = "default_max_concurrent_streams")]
    pub max_concurrent_streams: usize,

    #[serde(default)]
    pub role: super::NodeRole,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertise_url: Option<String>,

    #[serde(default = "default_max_in_flight")]
    pub max_in_flight: Option<std::num::NonZeroU32>,

    #[serde(default = "default_max_probe_in_flight")]
    pub max_probe_in_flight: std::num::NonZeroU32,

    #[serde(
        default = "default_max_route_body_bytes",
        deserialize_with = "deserialize_route_body_ceiling"
    )]
    pub max_route_body_bytes: usize,

    #[serde(
        default,
        deserialize_with = "deserialize_trusted_proxies",
        serialize_with = "serialize_trusted_proxies"
    )]
    #[schemars(with = "Vec<String>")]
    pub trusted_proxies: Vec<IpNet>,

    #[serde(default)]
    pub analytics_queue: AnalyticsQueueConfig,

    #[serde(default)]
    pub shutdown: super::ShutdownConfig,
}

/// `server.analytics_queue`: the request-analytics write queue. `capacity`
/// bounds the writes held at once (past it a write is dropped and counted);
/// `writers` bounds how many run concurrently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsQueueConfig {
    #[serde(default = "default_analytics_queue_capacity")]
    pub capacity: std::num::NonZeroU32,
    #[serde(default = "default_analytics_writers")]
    pub writers: std::num::NonZeroU32,
}

impl Default for AnalyticsQueueConfig {
    fn default() -> Self {
        Self {
            capacity: default_analytics_queue_capacity(),
            writers: default_analytics_writers(),
        }
    }
}

const DEFAULT_ANALYTICS_QUEUE_CAPACITY: std::num::NonZeroU32 =
    std::num::NonZeroU32::new(8192).expect("the default analytics queue capacity is non-zero");

const DEFAULT_ANALYTICS_WRITERS: std::num::NonZeroU32 =
    std::num::NonZeroU32::new(8).expect("the default analytics writer count is non-zero");

const fn default_analytics_queue_capacity() -> std::num::NonZeroU32 {
    DEFAULT_ANALYTICS_QUEUE_CAPACITY
}

const fn default_analytics_writers() -> std::num::NonZeroU32 {
    DEFAULT_ANALYTICS_WRITERS
}

impl ServerConfig {
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16, external_url: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port,
            external_url: external_url.into(),
            extra_cors_origins: Vec::new(),
            api_server_url: String::new(),
            api_internal_url: String::new(),
            api_external_url: String::new(),
            use_https: false,
            cors_allowed_origins: Vec::new(),
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            metrics_port: None,
            max_concurrent_streams: default_max_concurrent_streams(),
            role: super::NodeRole::default(),
            advertise_url: None,
            max_in_flight: default_max_in_flight(),
            max_probe_in_flight: default_max_probe_in_flight(),
            max_route_body_bytes: default_max_route_body_bytes(),
            trusted_proxies: Vec::new(),
            analytics_queue: AnalyticsQueueConfig::default(),
            shutdown: super::ShutdownConfig::default(),
        }
    }
}

pub const DEFAULT_MAX_IN_FLIGHT: std::num::NonZeroU32 =
    std::num::NonZeroU32::new(4096).expect("the default in-flight ceiling is non-zero");

#[expect(
    clippy::unnecessary_wraps,
    reason = "serde's default for an Option field: an omitted key is the default ceiling, an \
              explicit null is no ceiling"
)]
const fn default_max_in_flight() -> Option<std::num::NonZeroU32> {
    Some(DEFAULT_MAX_IN_FLIGHT)
}

pub const DEFAULT_MAX_PROBE_IN_FLIGHT: std::num::NonZeroU32 =
    std::num::NonZeroU32::new(64).expect("the default probe in-flight ceiling is non-zero");

const fn default_max_probe_in_flight() -> std::num::NonZeroU32 {
    DEFAULT_MAX_PROBE_IN_FLIGHT
}

pub const DEFAULT_MAX_ROUTE_BODY_BYTES: usize = 16 * 1024 * 1024;

pub const MAX_ROUTE_BODY_BYTES_LIMIT: usize = 1024 * 1024 * 1024;

const fn default_max_route_body_bytes() -> usize {
    DEFAULT_MAX_ROUTE_BODY_BYTES
}

#[derive(Debug, thiserror::Error)]
#[error(
    "server.max_route_body_bytes is {0}; it must be between 1 and {MAX_ROUTE_BODY_BYTES_LIMIT}"
)]
struct InvalidRouteBodyCeiling(usize);

fn deserialize_route_body_ceiling<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let ceiling = usize::deserialize(deserializer)?;
    if ceiling == 0 || ceiling > MAX_ROUTE_BODY_BYTES_LIMIT {
        return Err(serde::de::Error::custom(InvalidRouteBodyCeiling(ceiling)));
    }
    Ok(ceiling)
}

#[derive(Debug, thiserror::Error)]
#[error("'{0}' is not a valid CIDR range or IP address")]
struct InvalidTrustedProxy(String);

fn parse_trusted_proxy(entry: &str) -> Result<IpNet, InvalidTrustedProxy> {
    let trimmed = entry.trim();
    if let Ok(net) = trimmed.parse::<IpNet>() {
        return Ok(net);
    }
    match trimmed.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => Ok(IpNet::from(ipnet::Ipv4Net::from(v4))),
        Ok(IpAddr::V6(v6)) => Ok(IpNet::from(ipnet::Ipv6Net::from(v6))),
        Err(_) => Err(InvalidTrustedProxy(trimmed.to_owned())),
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

fn deserialize_instance_id<'de, D>(deserializer: D) -> Result<Option<InstanceId>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| InstanceId::try_new(id).map_err(serde::de::Error::custom))
        .transpose()
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
