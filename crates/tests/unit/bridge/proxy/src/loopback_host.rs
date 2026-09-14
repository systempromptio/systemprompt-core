//! Loopback detection for the `Host` header and for a configured proxy URL:
//! IPv4, IPv6 (bracketed, with and without a port) and `localhost` all count;
//! anything else — and anything unparseable — never does.

use systemprompt_bridge::proxy_probe::{PortMatch, classify_configured_port, host_is_loopback};

#[test]
fn ipv4_ipv6_and_localhost_authorities_are_loopback_with_or_without_a_port() {
    for host in [
        "127.0.0.1",
        "127.0.0.1:48217",
        "127.5.5.5:1",
        "localhost",
        "LocalHost:48217",
        "[::1]",
        "[::1]:48217",
    ] {
        assert!(host_is_loopback(host), "{host}");
    }
}

#[test]
fn public_addresses_and_malformed_authorities_are_not_loopback() {
    for host in [
        "10.0.0.1:48217",
        "[2001:db8::1]:48217",
        "gateway.example.com",
        "localhost.example.com",
        "",
        "not a host",
        "[::1",
    ] {
        assert!(!host_is_loopback(host), "{host}");
    }
}

#[test]
fn a_bracketed_ipv6_loopback_url_classifies_like_ipv4() {
    assert_eq!(
        classify_configured_port("http://[::1]:48217", 48217),
        PortMatch::Match
    );
    assert_eq!(
        classify_configured_port("http://[::1]:48218", 48217),
        PortMatch::Mismatch { configured: 48218 }
    );
}

#[test]
fn a_url_whose_port_is_not_a_number_is_unparseable_not_a_mismatch() {
    assert_eq!(
        classify_configured_port("http://127.0.0.1:abc", 48217),
        PortMatch::Unparseable
    );
    assert_eq!(
        classify_configured_port("http://127.0.0.1:70000", 48217),
        PortMatch::Unparseable
    );
}
