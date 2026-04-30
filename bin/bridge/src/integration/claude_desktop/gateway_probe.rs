use std::time::Instant;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct GatewayHealth {
    pub url: Option<String>,
    pub state: GatewayProbeState,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub enum GatewayProbeState {
    #[default]
    Unknown,
    Unconfigured,
    Listening,
    Refused,
    Timeout,
    HttpError,
}

pub(super) fn probe_gateway(url: &str) -> GatewayHealth {
    let started = Instant::now();

    let (host, port) = match parse_host_port(url) {
        Ok(v) => v,
        Err(e) => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(e),
                ..Default::default()
            };
        },
    };

    let addr = format!("{host}:{port}");
    let resolved = match resolve_first(&addr) {
        Some(a) => a,
        None => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(format!("cannot resolve {addr}")),
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
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::Refused,
                error: Some(e.to_string()),
                latency_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
                ..Default::default()
            };
        },
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::Timeout,
                error: Some(e.to_string()),
                latency_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
                ..Default::default()
            };
        },
        Err(e) => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(e.to_string()),
                latency_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
                ..Default::default()
            };
        },
    };

    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = stream.shutdown(std::net::Shutdown::Both);

    GatewayHealth {
        url: Some(url.to_string()),
        state: GatewayProbeState::Listening,
        http_status: None,
        latency_ms: Some(latency_ms),
        error: None,
    }
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
