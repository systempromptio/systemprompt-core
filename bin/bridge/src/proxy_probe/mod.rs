//! TCP/HTTP probes verifying the loopback proxy is reachable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
mod http;

use http::{elapsed_ms, http_head_status, parse_host_port, resolve_first};

pub(crate) use http::{http_get_body, resolve_first as resolve_first_addr};

use std::time::Instant;

use serde::Serialize;

use crate::verdict::{Tone, Verdict};

#[derive(Debug, Clone, Serialize, Default)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct ProxyHealth {
    pub url: Option<String>,
    pub state: ProxyProbeState,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum ProxyProbeState {
    #[default]
    Unknown,
    Unconfigured,
    Listening,
    Refused,
    Timeout,
    HttpError,
}

impl ProxyProbeState {
    #[must_use]
    pub const fn tone(self) -> Tone {
        match self {
            Self::Listening => Tone::Ok,
            Self::Unconfigured => Tone::Warn,
            Self::Unknown => Tone::Unknown,
            Self::Refused | Self::Timeout | Self::HttpError => Tone::Err,
        }
    }

    #[must_use]
    pub const fn governing(self) -> bool {
        matches!(self, Self::Listening)
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict<Self> {
        Verdict::new(self.tone(), self)
    }
}

#[must_use]
pub fn probe(url: Option<&str>) -> ProxyHealth {
    let probed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let Some(url) = url.filter(|s| !s.is_empty()) else {
        return ProxyHealth {
            state: ProxyProbeState::Unconfigured,
            probed_at_unix,
            ..Default::default()
        };
    };

    let started = Instant::now();
    match attempt(url, started) {
        Ok((http_status, latency_ms)) => ProxyHealth {
            url: Some(url.to_owned()),
            state: ProxyProbeState::Listening,
            http_status: Some(http_status),
            latency_ms: Some(latency_ms),
            error: None,
            probed_at_unix,
        },
        Err(failure) => ProxyHealth {
            url: Some(url.to_owned()),
            state: failure.state,
            error: Some(failure.error),
            latency_ms: failure.latency_ms,
            probed_at_unix,
            ..Default::default()
        },
    }
}

struct ProbeFailure {
    state: ProxyProbeState,
    error: String,
    latency_ms: Option<u64>,
}

impl ProbeFailure {
    fn http_error(error: String, started: Instant) -> Self {
        Self {
            state: ProxyProbeState::HttpError,
            error,
            latency_ms: Some(elapsed_ms(started)),
        }
    }
}

fn attempt(url: &str, started: Instant) -> Result<(u16, u64), ProbeFailure> {
    let (host, port) = parse_host_port(url).map_err(|e| ProbeFailure {
        state: ProxyProbeState::HttpError,
        error: e.to_string(),
        latency_ms: None,
    })?;

    let addr = format!("{host}:{port}");
    let resolved = resolve_first(&addr).ok_or_else(|| ProbeFailure {
        state: ProxyProbeState::HttpError,
        error: format!("cannot resolve {addr}"),
        latency_ms: None,
    })?;

    let mut stream =
        std::net::TcpStream::connect_timeout(&resolved, std::time::Duration::from_millis(1500))
            .map_err(|e| {
                let state = match e.kind() {
                    std::io::ErrorKind::ConnectionRefused => ProxyProbeState::Refused,
                    std::io::ErrorKind::TimedOut => ProxyProbeState::Timeout,
                    _ => ProxyProbeState::HttpError,
                };
                ProbeFailure {
                    state,
                    error: e.to_string(),
                    latency_ms: Some(elapsed_ms(started)),
                }
            })?;

    let http_status = http_head_status(&mut stream, &host)
        .map_err(|e| ProbeFailure::http_error(e.to_string(), started))?;
    Ok((http_status, elapsed_ms(started)))
}

/// How a written client config compares to the port the proxy actually holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortMatch {
    Match,
    Mismatch { configured: u16 },
    NotLoopback,
    Unparseable,
}

#[must_use]
pub fn classify_configured_port(configured_url: &str, actual: u16) -> PortMatch {
    let Ok((host, port)) = parse_host_port(configured_url) else {
        return PortMatch::Unparseable;
    };
    if !host_is_loopback(&host) {
        return PortMatch::NotLoopback;
    }
    if port == actual {
        PortMatch::Match
    } else {
        PortMatch::Mismatch { configured: port }
    }
}

#[must_use]
pub fn host_is_loopback(host: &str) -> bool {
    let Ok(authority) = host.parse::<::http::uri::Authority>() else {
        return false;
    };
    let name = authority.host().trim_matches(|c| c == '[' || c == ']');
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }
    name.parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
}
