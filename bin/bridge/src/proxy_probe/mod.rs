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

    let (host, port) = match parse_host_port(url) {
        Ok(v) => v,
        Err(e) => {
            return ProxyHealth {
                url: Some(url.to_owned()),
                state: ProxyProbeState::HttpError,
                error: Some(e.to_string()),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let addr = format!("{host}:{port}");
    let Some(resolved) = resolve_first(&addr) else {
        return ProxyHealth {
            url: Some(url.to_owned()),
            state: ProxyProbeState::HttpError,
            error: Some(format!("cannot resolve {addr}")),
            probed_at_unix,
            ..Default::default()
        };
    };

    let mut stream = match std::net::TcpStream::connect_timeout(
        &resolved,
        std::time::Duration::from_millis(1500),
    ) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            return ProxyHealth {
                url: Some(url.to_owned()),
                state: ProxyProbeState::Refused,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            return ProxyHealth {
                url: Some(url.to_owned()),
                state: ProxyProbeState::Timeout,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
        Err(e) => {
            return ProxyHealth {
                url: Some(url.to_owned()),
                state: ProxyProbeState::HttpError,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let http_status = match http_head_status(&mut stream, &host) {
        Ok(s) => s,
        Err(e) => {
            return ProxyHealth {
                url: Some(url.to_owned()),
                state: ProxyProbeState::HttpError,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let latency_ms = elapsed_ms(started);
    _ = stream.shutdown(std::net::Shutdown::Both);

    ProxyHealth {
        url: Some(url.to_owned()),
        state: ProxyProbeState::Listening,
        http_status: Some(http_status),
        latency_ms: Some(latency_ms),
        error: None,
        probed_at_unix,
    }
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
    if host != "127.0.0.1" && host != "localhost" && host != "::1" {
        return PortMatch::NotLoopback;
    }
    if port == actual {
        PortMatch::Match
    } else {
        PortMatch::Mismatch { configured: port }
    }
}
