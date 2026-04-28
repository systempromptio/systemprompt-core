use std::net::TcpStream;
use std::time::Duration;

use ureq::AgentBuilder;

use crate::http_local::response::write_chunked;
use crate::http_local::{ResponseBuilder, is_hop_by_hop};
use crate::proxy::server::Request;
use crate::{auth, config};

const REFRESH_THRESHOLD_SECS: u64 = 300;

pub fn forward(req: &Request, gateway_base: &str, client: &mut TcpStream) -> Result<u16, String> {
    let cfg = config::load();
    let token_out = auth::read_or_refresh(&cfg, REFRESH_THRESHOLD_SECS)
        .ok_or_else(|| "no JWT available — sign in via cowork GUI".to_string())?;

    let target = req.target();
    let url = format!(
        "{}{}",
        gateway_base.trim_end_matches('/'),
        if target.starts_with('/') {
            target.clone()
        } else {
            format!("/{}", target)
        }
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
        if is_hop_by_hop(name) {
            continue;
        }
        request = request.set(name, value);
    }
    request = request.set(
        "authorization",
        &format!("Bearer {}", token_out.token.expose()),
    );
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
            ResponseBuilder::new(502)
                .content_type("text/plain")
                .body(msg.as_bytes())
                .write(client)
                .map_err(|e| format!("write status: {e}"))?;
            return Ok(502);
        },
    };

    let status = resp.status();
    forward_response(client, status, resp)?;
    Ok(status)
}

fn forward_response(
    client: &mut TcpStream,
    status: u16,
    resp: ureq::Response,
) -> Result<(), String> {
    let reason = resp.status_text().to_string();
    let header_names: Vec<String> = resp.headers_names();

    let mut headers: Vec<(String, String)> = Vec::with_capacity(header_names.len());
    for name in &header_names {
        if is_response_hop_by_hop(name) {
            continue;
        }
        if let Some(value) = resp.header(name) {
            headers.push((name.clone(), value.to_string()));
        }
    }

    let mut reader = resp.into_reader();
    write_chunked(client, status, &reason, &headers, &mut reader)
}

fn is_response_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "upgrade"
    )
}
