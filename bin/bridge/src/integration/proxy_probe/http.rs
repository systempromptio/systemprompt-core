//! Minimal blocking HTTP/1.1 exchanges used by the loopback probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

pub(super) fn http_get_body(
    stream: &mut std::net::TcpStream,
    host: &str,
    path: &str,
) -> Result<String, String> {
    use std::io::{Read, Write};
    _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(1500)));
    _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(1500)));
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: \
         application/json\r\nUser-Agent: systemprompt-bridge-probe\r\n\r\n",
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write probe: {e}"))?;

    // Why: bounded because an unrelated service on this port could stream
    // forever.
    let mut raw = Vec::new();
    let mut chunk = [0u8; 1024];
    while raw.len() < 8192 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => raw.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(format!("read probe: {e}")),
        }
    }
    let text = String::from_utf8_lossy(&raw);
    let status_ok = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .is_some_and(|c| c == 200);
    if !status_ok {
        return Err("identity endpoint did not return 200".to_owned());
    }
    text.split_once("\r\n\r\n")
        .map(|(_, body)| body.trim().to_owned())
        .ok_or_else(|| "no body in identity response".to_owned())
}

pub(super) fn http_head_status(
    stream: &mut std::net::TcpStream,
    host: &str,
) -> Result<u16, String> {
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

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn resolve_first(addr: &str) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok()?.next()
}

pub(super) fn parse_host_port(raw: &str) -> Result<(String, u16), String> {
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
