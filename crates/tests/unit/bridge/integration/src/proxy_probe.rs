use systemprompt_bridge::integration::{ProxyProbeState, proxy_probe};

#[test]
fn no_url_yields_unconfigured_state() {
    let health = proxy_probe::probe(None);
    assert_eq!(health.state, ProxyProbeState::Unconfigured);
    assert!(health.url.is_none());
}

#[test]
fn empty_url_yields_unconfigured_state() {
    let health = proxy_probe::probe(Some(""));
    assert_eq!(health.state, ProxyProbeState::Unconfigured);
}

#[test]
fn missing_scheme_yields_http_error() {
    let health = proxy_probe::probe(Some("localhost:8080"));
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.is_some());
}

#[test]
fn refused_connection_to_closed_port_yields_refused() {
    let health = proxy_probe::probe(Some("http://127.0.0.1:1"));
    assert!(
        matches!(
            health.state,
            ProxyProbeState::Refused | ProxyProbeState::HttpError | ProxyProbeState::Timeout
        ),
        "unexpected state: {:?}",
        health.state,
    );
}

fn spawn_http_responder(
    response: &'static str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    (addr, handle)
}

#[test]
fn listening_endpoint_yields_listening_with_status() {
    let (addr, handle) = spawn_http_responder("HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
    let url = format!("http://127.0.0.1:{}/", addr.port());
    let health = proxy_probe::probe(Some(&url));
    handle.join().unwrap();
    assert_eq!(health.state, ProxyProbeState::Listening);
    assert_eq!(health.http_status, Some(200));
    assert!(health.latency_ms.is_some());
    assert!(health.error.is_none());
}

#[test]
fn error_status_from_endpoint_is_still_listening() {
    let (addr, handle) = spawn_http_responder("HTTP/1.1 503 Service Unavailable\r\n\r\n");
    let url = format!("http://127.0.0.1:{}", addr.port());
    let health = proxy_probe::probe(Some(&url));
    handle.join().unwrap();
    assert_eq!(health.state, ProxyProbeState::Listening);
    assert_eq!(health.http_status, Some(503));
}

#[test]
fn short_response_yields_http_error() {
    let (addr, handle) = spawn_http_responder("HT");
    let url = format!("http://127.0.0.1:{}", addr.port());
    let health = proxy_probe::probe(Some(&url));
    handle.join().unwrap();
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.unwrap().contains("short response"));
}

#[test]
fn garbage_status_line_yields_http_error() {
    let (addr, handle) = spawn_http_responder("HTTP/1.1 notanumber OK stuffing bytes\r\n\r\n");
    let url = format!("http://127.0.0.1:{}", addr.port());
    let health = proxy_probe::probe(Some(&url));
    handle.join().unwrap();
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.unwrap().contains("bad status code"));
}

#[test]
fn unsupported_scheme_yields_http_error() {
    let health = proxy_probe::probe(Some("ftp://127.0.0.1:2121"));
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.unwrap().contains("unsupported scheme"));
}

#[test]
fn unresolvable_host_yields_http_error() {
    let health = proxy_probe::probe(Some("http://nonexistent-host.invalid"));
    assert_eq!(health.state, ProxyProbeState::HttpError);
}

#[test]
fn missing_host_yields_http_error() {
    let health = proxy_probe::probe(Some("http:///path-only"));
    assert_eq!(health.state, ProxyProbeState::HttpError);
    assert!(health.error.unwrap().contains("missing host"));
}
