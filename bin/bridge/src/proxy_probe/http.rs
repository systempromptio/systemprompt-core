//! Minimal blocking HTTP/1.1 exchanges used by the loopback probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("write probe: {0}")]
    Write(#[source] std::io::Error),
    #[error("read probe: {0}")]
    Read(#[source] std::io::Error),
    #[error("identity endpoint did not return 200")]
    NotOk,
    #[error("no body in identity response")]
    NoBody,
    #[error("short response: {0} bytes")]
    Short(usize),
    #[error("non-utf8 status: {0}")]
    Utf8(#[source] std::str::Utf8Error),
    #[error("missing status code")]
    MissingStatus,
    #[error("bad status code '{code}': {source}")]
    BadStatus {
        code: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("missing scheme in {0}")]
    MissingScheme(String),
    #[error("unsupported scheme: {0}")]
    UnsupportedScheme(String),
    #[error("missing host")]
    MissingHost,
}

pub(crate) fn http_get_body(
    stream: &mut std::net::TcpStream,
    host: &str,
    path: &str,
) -> Result<String, ProbeError> {
    use std::io::{Read, Write};
    _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(1500)));
    _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(1500)));
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: \
         application/json\r\nUser-Agent: systemprompt-bridge-probe\r\n\r\n",
    );
    stream
        .write_all(req.as_bytes())
        .map_err(ProbeError::Write)?;

    // Why: bounded because an unrelated service on this port could stream
    // forever.
    let mut raw = Vec::new();
    let mut chunk = [0u8; 1024];
    while raw.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(ProbeError::Read(e)),
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let status_ok = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .is_some_and(|c| c == 200);
    if !status_ok {
        return Err(ProbeError::NotOk);
    }
    text.split_once("\r\n\r\n")
        .map(|(_, body)| body.trim().to_owned())
        .ok_or(ProbeError::NoBody)
}

pub(super) fn http_head_status(
    stream: &mut std::net::TcpStream,
    host: &str,
) -> Result<u16, ProbeError> {
    use std::io::{Read, Write};
    _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(1500)));
    _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(1500)));
    let req = format!(
        "HEAD /healthz HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: \
         systemprompt-bridge-probe\r\n\r\n",
    );
    stream
        .write_all(req.as_bytes())
        .map_err(ProbeError::Write)?;
    let mut buf = [0u8; 64];
    let n = stream.read(&mut buf).map_err(ProbeError::Read)?;
    if n < 12 {
        return Err(ProbeError::Short(n));
    }
    let line = std::str::from_utf8(&buf[..n]).map_err(ProbeError::Utf8)?;
    let mut parts = line.split_whitespace();
    let _version = parts.next();
    let code = parts.next().ok_or(ProbeError::MissingStatus)?;
    code.parse::<u16>().map_err(|source| ProbeError::BadStatus {
        code: code.to_owned(),
        source,
    })
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn resolve_first(addr: &str) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok()?.next()
}

pub(super) fn parse_host_port(raw: &str) -> Result<(String, u16), ProbeError> {
    let Some((scheme, rest)) = raw.split_once("://") else {
        return Err(ProbeError::MissingScheme(raw.to_owned()));
    };
    let default_port: u16 = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        other => return Err(ProbeError::UnsupportedScheme(other.to_owned())),
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err(ProbeError::MissingHost);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_owned(), p.parse::<u16>().unwrap_or(default_port)),
        None => (authority.to_owned(), default_port),
    };
    Ok((host, port))
}
