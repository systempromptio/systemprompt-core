//! The `ClientIp` axum extractor.
//!
//! `resolve_client_ip_from_config` is tested directly, but the extractor that
//! wraps it — the only way a handler ever obtains the caller's address — is
//! never driven. Handlers use the value for rate limiting, IP bans and session
//! attribution, so an extractor that silently yields `None`, or that trusts a
//! forwarded header it should not, is a security-relevant failure.

use std::net::{IpAddr, SocketAddr};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::routing::get;
use systemprompt_api::services::middleware::client_addr::ClientIp;
use tower::ServiceExt;

async fn echo_ip(ClientIp(ip): ClientIp) -> String {
    ip.map_or_else(|| "none".to_owned(), |a| a.to_string())
}

async fn resolved(peer: Option<SocketAddr>, headers: &[(&str, &str)]) -> String {
    let app = Router::new().route("/", get(echo_ip));
    let mut builder = Request::builder().uri("/");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let mut req = builder.body(Body::empty()).expect("request must build");
    if let Some(peer) = peer {
        req.extensions_mut().insert(ConnectInfo(peer));
    }

    let resp = app.oneshot(req).await.expect("request must complete");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body is small");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn peer(ip: [u8; 4]) -> SocketAddr {
    SocketAddr::from((ip, 51234))
}

#[tokio::test]
async fn the_extractor_yields_the_peer_address_when_no_headers_are_present() {
    assert_eq!(
        resolved(Some(peer([203, 0, 113, 7])), &[]).await,
        "203.0.113.7"
    );
}

#[tokio::test]
async fn a_request_with_no_peer_and_no_headers_yields_no_address() {
    assert_eq!(
        resolved(None, &[]).await,
        "none",
        "the extractor must report absence rather than inventing an address"
    );
}

#[tokio::test]
async fn an_untrusted_peer_cannot_spoof_its_address_with_a_forwarded_header() {
    // The fixture profile declares no trusted proxies, so a hop header from an
    // arbitrary peer must be ignored entirely — honouring it would let any
    // caller evade an IP ban or another caller's rate-limit bucket.
    let resolved = resolved(
        Some(peer([203, 0, 113, 7])),
        &[("x-forwarded-for", "198.51.100.9")],
    )
    .await;

    assert_eq!(
        resolved, "203.0.113.7",
        "a forwarded header from an untrusted peer must not override the socket address"
    );
}

#[tokio::test]
async fn a_forwarded_header_alone_does_not_produce_an_address() {
    assert_eq!(
        resolved(None, &[("x-forwarded-for", "198.51.100.9")]).await,
        "none",
        "with no peer to trust, a claimed address is not an address"
    );
}

#[tokio::test]
async fn the_extracted_address_parses_back_to_the_peer_it_came_from() {
    let text = resolved(Some(peer([10, 1, 2, 3])), &[]).await;

    let parsed: IpAddr = text.parse().expect("the extractor emits a real address");
    assert_eq!(parsed, IpAddr::from([10, 1, 2, 3]));
}
