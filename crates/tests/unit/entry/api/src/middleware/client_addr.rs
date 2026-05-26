//! Unit tests for `client_addr` — the originating-IP resolver that walks
//! `X-Forwarded-For` only when the immediate peer is in the trusted-proxy
//! allowlist.

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, HeaderValue};
use ipnet::IpNet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use systemprompt_api::services::middleware::client_addr::{
    parse_trusted_proxies, resolve_client_ip,
};

fn sock(ip: &str) -> ConnectInfo<SocketAddr> {
    ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 1234))
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

#[test]
fn parse_trusted_proxies_handles_cidrs_and_bare_ips() {
    let raw = vec![
        "10.0.0.0/8".to_owned(),
        "192.168.1.1".to_owned(),
        "2001:db8::/32".to_owned(),
        "::1".to_owned(),
    ];
    let parsed = parse_trusted_proxies(&raw);
    assert_eq!(parsed.len(), 4);
    assert!(parsed[0].contains(&ip("10.5.5.5")));
    assert!(parsed[1].contains(&ip("192.168.1.1")));
    assert!(!parsed[1].contains(&ip("192.168.1.2")));
}

#[test]
fn parse_trusted_proxies_drops_invalid_entries() {
    let raw = vec![
        "10.0.0.0/8".to_owned(),
        "not-an-ip".to_owned(),
        "".to_owned(),
        "   ".to_owned(),
        "1.2.3.4/99".to_owned(),
    ];
    let parsed = parse_trusted_proxies(&raw);
    assert_eq!(parsed.len(), 1);
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
    let trusted = parse_trusted_proxies(&["10.0.0.0/8".to_owned()]);
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
    let trusted = parse_trusted_proxies(&["10.0.0.0/8".to_owned()]);
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
    let trusted = parse_trusted_proxies(&["10.0.0.0/8".to_owned()]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.42"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("198.51.100.42"));
}

#[test]
fn trusted_peer_honours_cf_connecting_ip_when_no_others() {
    let trusted = parse_trusted_proxies(&["10.0.0.0/8".to_owned()]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.99"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("203.0.113.99"));
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
    let trusted = parse_trusted_proxies(&["10.0.0.0/8".to_owned()]);
    let peer = sock("10.0.0.1");
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("not-an-ip,,"));
    let resolved = resolve_client_ip(&headers, Some(&peer), &trusted).unwrap();
    assert_eq!(resolved, ip("10.0.0.1"));
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
