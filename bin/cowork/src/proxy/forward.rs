use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use ureq::AgentBuilder;

use crate::auth;
use crate::config;
use crate::proxy::server::Request;

const REFRESH_THRESHOLD_SECS: u64 = 300;
const HOP_BY_HOP: &[&str] = &[
    "host",
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "authorization",
    "x-api-key",
];

pub fn forward(req: &Request, gateway_base: &str, client: &mut TcpStream) -> Result<u16, String> {
    let cfg = config::load();
    let token_out = auth::read_or_refresh(&cfg, REFRESH_THRESHOLD_SECS)
        .ok_or_else(|| "no JWT available — sign in via cowork GUI".to_string())?;

    let url = format!(
        "{}{}",
        gateway_base.trim_end_matches('/'),
        if req.path.starts_with('/') { req.path.clone() } else { format!("/{}", req.path) }
    );

    let agent = AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build();

    let mut request = match req.method.as_str() {
        "GET" => agent.get(&url),
        "POST" => agent.post(&url),
        "PUT" => agent.put(&url),
        "DELETE" => agent.delete(&url),
        "PATCH" => agent.request("PATCH", &url),
        "HEAD" => agent.request("HEAD", &url),
        other => agent.request(other, &url),
    };

    for (name, value) in &req.headers {
        if HOP_BY_HOP.iter().any(|h| h.eq_ignore_ascii_case(name)) {
            continue;
        }
        request = request.set(name, value);
    }
    request = request.set("authorization", &format!("Bearer {}", token_out.token.expose()));
    request = request.set("x-cowork-proxied", "1");

    let response = if req.body.is_empty() {
        request.call()
    } else {
        request.send_bytes(&req.body)
    };

    let resp = match response {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            forward_response(client, code, r)?;
            return Ok(code);
        },
        Err(e) => {
            let msg = format!("upstream error: {e}");
            write_status(client, 502, "text/plain", msg.as_bytes())?;
            return Ok(502);
        },
    };

    let status = resp.status();
    forward_response(client, status, resp)?;
    Ok(status)
}

fn forward_response(client: &mut TcpStream, status: u16, resp: ureq::Response) -> Result<(), String> {
    let reason = resp.status_text().to_string();
    let header_names: Vec<String> = resp.headers_names();

    let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
    let mut had_content_length = false;
    let mut had_transfer_encoding = false;
    for name in &header_names {
        if name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("keep-alive")
            || name.eq_ignore_ascii_case("proxy-authenticate")
            || name.eq_ignore_ascii_case("proxy-authorization")
            || name.eq_ignore_ascii_case("te")
            || name.eq_ignore_ascii_case("trailers")
            || name.eq_ignore_ascii_case("upgrade")
        {
            continue;
        }
        if name.eq_ignore_ascii_case("content-length") {
            had_content_length = true;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            had_transfer_encoding = true;
        }
        if let Some(value) = resp.header(name) {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
    }
    let stream_body = !had_content_length;
    if stream_body && !had_transfer_encoding {
        head.push_str("Transfer-Encoding: chunked\r\n");
    }
    head.push_str("Connection: close\r\n\r\n");
    client
        .write_all(head.as_bytes())
        .map_err(|e| format!("write header: {e}"))?;
    client.flush().ok();

    let mut reader = resp.into_reader();
    let mut buf = [0u8; 4096];
    if stream_body && !had_transfer_encoding {
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk_header = format!("{:x}\r\n", n);
                    client
                        .write_all(chunk_header.as_bytes())
                        .map_err(|e| format!("write chunk header: {e}"))?;
                    client
                        .write_all(&buf[..n])
                        .map_err(|e| format!("write chunk body: {e}"))?;
                    client
                        .write_all(b"\r\n")
                        .map_err(|e| format!("write chunk crlf: {e}"))?;
                    client.flush().ok();
                },
                Err(e) => return Err(format!("upstream read: {e}")),
            }
        }
        client
            .write_all(b"0\r\n\r\n")
            .map_err(|e| format!("write final chunk: {e}"))?;
        client.flush().ok();
    } else {
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    client
                        .write_all(&buf[..n])
                        .map_err(|e| format!("write body: {e}"))?;
                    client.flush().ok();
                },
                Err(e) => return Err(format!("upstream read: {e}")),
            }
        }
    }
    Ok(())
}

fn write_status(client: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) -> Result<(), String> {
    let header = format!(
        "HTTP/1.1 {status} Bad Gateway\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    client
        .write_all(header.as_bytes())
        .map_err(|e| format!("write status: {e}"))?;
    client
        .write_all(body)
        .map_err(|e| format!("write status body: {e}"))?;
    client.flush().ok();
    Ok(())
}
