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

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
pub enum ProxyProbeState {
    #[default]
    Unknown,
    Unconfigured,
    Listening,
    Refused,
    Timeout,
    HttpError,
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
                error: Some(e),
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
                error: Some(e),
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

fn http_head_status(stream: &mut std::net::TcpStream, host: &str) -> Result<u16, String> {
    use std::io::{Read, Write};
    _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(1500)));
    _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(1500)));
    let req = format!(
        "HEAD /healthz HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: \
         systemprompt-bridge-probe\r\n\r\n",
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write probe: {e}"))?;
    let mut buf = [0u8; 64];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("read probe: {e}"))?;
    if n < 12 {
        return Err(format!("short response: {n} bytes"));
    }
    let line = std::str::from_utf8(&buf[..n]).map_err(|e| format!("non-utf8 status: {e}"))?;
    let mut parts = line.split_whitespace();
    let _version = parts.next();
    let code = parts
        .next()
        .ok_or_else(|| "missing status code".to_owned())?;
    code.parse::<u16>()
        .map_err(|e| format!("bad status code '{code}': {e}"))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn resolve_first(addr: &str) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok()?.next()
}

fn parse_host_port(raw: &str) -> Result<(String, u16), String> {
    let Some((scheme, rest)) = raw.split_once("://") else {
        return Err(format!("missing scheme in {raw}"));
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
        Some((h, p)) => (h.to_owned(), p.parse::<u16>().unwrap_or(default_port)),
        None => (authority.to_owned(), default_port),
    };
    Ok((host, port))
}
