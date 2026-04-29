use std::time::Instant;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProxyHealth {
    pub url: Option<String>,
    pub state: ProxyProbeState,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub enum ProxyProbeState {
    #[default]
    Unknown,
    Unconfigured,
    Listening,
    Refused,
    Timeout,
    HttpError,
}

pub fn probe(url: Option<&str>) -> ProxyHealth {
    let probed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
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
                url: Some(url.to_string()),
                state: ProxyProbeState::HttpError,
                error: Some(e),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let addr = format!("{host}:{port}");
    let resolved = match resolve_first(&addr) {
        Some(a) => a,
        None => {
            return ProxyHealth {
                url: Some(url.to_string()),
                state: ProxyProbeState::HttpError,
                error: Some(format!("cannot resolve {addr}")),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let stream = match std::net::TcpStream::connect_timeout(
        &resolved,
        std::time::Duration::from_millis(1500),
    ) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            return ProxyHealth {
                url: Some(url.to_string()),
                state: ProxyProbeState::Refused,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            return ProxyHealth {
                url: Some(url.to_string()),
                state: ProxyProbeState::Timeout,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
        Err(e) => {
            return ProxyHealth {
                url: Some(url.to_string()),
                state: ProxyProbeState::HttpError,
                error: Some(e.to_string()),
                latency_ms: Some(elapsed_ms(started)),
                probed_at_unix,
                ..Default::default()
            };
        },
    };

    let latency_ms = elapsed_ms(started);
    let _ = stream.shutdown(std::net::Shutdown::Both);

    ProxyHealth {
        url: Some(url.to_string()),
        state: ProxyProbeState::Listening,
        http_status: None,
        latency_ms: Some(latency_ms),
        error: None,
        probed_at_unix,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn resolve_first(addr: &str) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok()?.next()
}

fn parse_host_port(raw: &str) -> Result<(String, u16), String> {
    let (scheme, rest) = match raw.split_once("://") {
        Some(v) => v,
        None => return Err(format!("missing scheme in {raw}")),
    };
    let default_port: u16 = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        other => return Err(format!("unsupported scheme: {other}")),
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err("missing host".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(default_port)),
        None => (authority.to_string(), default_port),
    };
    Ok((host, port))
}
