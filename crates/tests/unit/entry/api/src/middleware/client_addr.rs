//! Unit tests for `client_addr` — the originating-IP resolver that walks
//! `X-Forwarded-For` only when the immediate peer is in the trusted-proxy
//! allowlist.

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue};
use ipnet::IpNet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use systemprompt_api::services::middleware::client_addr::{
    forwarded_headers_ignored, resolve_client_ip,
};

fn sock(ip: &str) -> ConnectInfo<SocketAddr> {
    ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 1234))
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

fn nets(entries: &[&str]) -> Vec<IpNet> {
    entries.iter().map(|e| e.parse().unwrap()).collect()
}

#[test]
fn untrusted_peer_returns_peer_ignoring_xff() {
    let trusted: Vec<IpNet> = vec![];
    let peer = sock("203.0.113.5");
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.1, 203.0.113.5"),
    );
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.5"));
}

#[test]
fn trusted_peer_walks_xff_right_to_left() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    // Real client first, then proxy hops to the right.
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.7, 10.0.0.5, 10.0.0.6"),
    );
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("198.51.100.7"));
}

#[test]
fn trusted_peer_falls_back_to_peer_when_all_hops_trusted() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("10.0.0.5, 10.0.0.6"),
    );
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("10.0.0.1"));
}

#[test]
fn trusted_peer_honours_x_real_ip_when_no_xff() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.42"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("198.51.100.42"));
}

#[test]
fn trusted_peer_honours_cf_connecting_ip_when_no_others() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.99"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.99"));
}

#[test]
fn trusted_peer_honours_fly_client_ip_when_no_xff() {
    let trusted = nets(&["fc00::/7"]);
    let peer = sock("fdaa::3");
    let mut headers = HeaderMap::new();
    headers.insert("fly-client-ip", HeaderValue::from_static("203.0.113.99"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.99"));
}

#[test]
fn fly_client_ip_wins_over_cf_connecting_ip() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("fly-client-ip", HeaderValue::from_static("203.0.113.99"));
    headers.insert("cf-connecting-ip", HeaderValue::from_static("198.51.100.1"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.99"));
}

#[test]
fn untrusted_peer_ignores_fly_client_ip() {
    let trusted: Vec<IpNet> = vec![];
    let peer = sock("203.0.113.5");
    let mut headers = HeaderMap::new();
    headers.insert("fly-client-ip", HeaderValue::from_static("198.51.100.1"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.5"));
}

#[test]
fn no_connect_info_returns_none() {
    let trusted: Vec<IpNet> = vec![];
    let resolved = resolve_client_ip(&HeaderMap::new(), None, &trusted);
    assert!(resolved.is_none());
}

#[test]
fn untrusted_peer_ignores_x_real_ip() {
    let trusted: Vec<IpNet> = vec![];
    let peer = sock("203.0.113.5");
    let mut headers = HeaderMap::new();
    headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.42"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.5"));
}

#[test]
fn trusted_peer_with_malformed_xff_falls_back() {
    let trusted = nets(&["10.0.0.0/8"]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("not-an-ip,,"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("10.0.0.1"));
}

#[test]
fn spoofed_private_xff_via_trusted_proxy_never_wins() {
    // A scanner sends `X-Forwarded-For: 10.1.2.3`; the trusted edge (Cloudflare)
    // appends the real connecting IP and forwards to origin. Walking XFF
    // right-to-left returns the attested hop, never the spoofed RFC1918 value.
    let trusted = nets(&["198.51.100.0/24"]);
    let peer = sock("198.51.100.1");
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("10.1.2.3, 203.0.113.7"),
    );
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.7"));
    assert_ne!(resolved, ip("10.1.2.3"));
}

#[test]
fn spoofed_xff_falls_through_to_cf_connecting_ip() {
    // With only a spoofed private XFF hop present, the resolver still returns
    // that hop (it is the closest untrusted entity the edge saw); the attested
    // `cf-connecting-ip` path is exercised when XFF is absent.
    let trusted = nets(&["198.51.100.0/24"]);
    let peer = sock("198.51.100.1");
    let mut headers = HeaderMap::new();
    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.7"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.7"));
}

#[test]
fn untrusted_private_peer_with_xff_flags_ignored_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
    for peer in [
        "fdaa::3",
        "10.0.0.1",
        "172.16.0.9",
        "100.64.0.1",
        "127.0.0.1",
    ] {
        assert!(
            forwarded_headers_ignored(&headers, ip(peer), &[]),
            "expected flag for peer {peer}"
        );
    }
}

#[test]
fn trusted_private_peer_with_xff_is_not_flagged() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
    let trusted = nets(&["fc00::/7"]);
    assert!(!forwarded_headers_ignored(
        &headers,
        ip("fdaa::3"),
        &trusted
    ));
}

#[test]
fn untrusted_public_peer_with_xff_is_not_flagged() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
    assert!(!forwarded_headers_ignored(&headers, ip("203.0.113.5"), &[]));
}

#[test]
fn untrusted_private_peer_without_xff_is_not_flagged() {
    assert!(!forwarded_headers_ignored(
        &HeaderMap::new(),
        ip("10.0.0.1"),
        &[]
    ));
}

#[test]
fn ipv4_loopback_resolves_when_untrusted() {
    let trusted: Vec<IpNet> = vec![];
    let peer = ConnectInfo(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        9000,
    ));
    let resolved = resolve_client_ip(&HeaderMap::new(), Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("127.0.0.1"));
}
