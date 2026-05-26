//! Unit tests for top-level crate helpers: `McpHttpConfig`, `SessionTimeouts`,
//! `McpState`, and the protocol-version accessors.

use std::time::Duration;
use systemprompt_mcp::{McpHttpConfig, SessionTimeouts, mcp_protocol_version, mcp_protocol_version_str};

#[test]
fn test_session_timeouts_default_is_none() {
    let t = SessionTimeouts::default();
    assert!(t.init.is_none());
    assert!(t.keep_alive.is_none());
}

#[test]
fn test_session_timeouts_copy() {
    let t = SessionTimeouts {
        init: Some(Duration::from_secs(5)),
        keep_alive: Some(Duration::from_secs(60)),
    };
    let t2 = t;
    assert_eq!(t.init, t2.init);
    assert_eq!(t.keep_alive, t2.keep_alive);
}

#[test]
fn test_session_timeouts_debug() {
    let t = SessionTimeouts::default();
    let d = format!("{:?}", t);
    assert!(d.contains("SessionTimeouts"));
}

#[test]
fn test_mcp_http_config_default_allows_localhost_variants() {
    let config = McpHttpConfig::default();
    let hosts = config.allowed_hosts.expect("default has hosts");
    assert!(hosts.iter().any(|h| h == "localhost"));
    assert!(hosts.iter().any(|h| h == "127.0.0.1"));
    assert!(hosts.iter().any(|h| h == "0.0.0.0"));
    assert!(hosts.iter().any(|h| h == "::1"));
    assert!(hosts.iter().any(|h| h == "::"));
}

#[test]
fn test_mcp_http_config_default_no_origins() {
    let config = McpHttpConfig::default();
    assert!(config.allowed_origins.is_empty());
}

#[test]
fn test_mcp_http_config_clone() {
    let config = McpHttpConfig::default();
    let cloned = config.clone();
    assert_eq!(cloned.allowed_origins, config.allowed_origins);
}

#[test]
fn test_mcp_http_config_debug() {
    let config = McpHttpConfig::default();
    let s = format!("{:?}", config);
    assert!(s.contains("McpHttpConfig"));
}

#[test]
fn test_mcp_http_config_custom() {
    let config = McpHttpConfig {
        allowed_hosts: None,
        allowed_origins: vec!["https://example.com".to_string()],
        session: SessionTimeouts {
            init: Some(Duration::from_secs(10)),
            keep_alive: None,
        },
    };
    assert!(config.allowed_hosts.is_none());
    assert_eq!(config.allowed_origins.len(), 1);
    assert_eq!(config.session.init, Some(Duration::from_secs(10)));
}

#[test]
fn test_protocol_version_string_nonempty() {
    let v = mcp_protocol_version();
    assert!(!v.is_empty());
}

#[test]
fn test_protocol_version_str_matches_owned() {
    let owned = mcp_protocol_version();
    let s = mcp_protocol_version_str();
    assert_eq!(owned, s);
}

#[test]
fn test_protocol_version_str_stable_across_calls() {
    let a = mcp_protocol_version_str();
    let b = mcp_protocol_version_str();
    assert_eq!(a, b);
    // Same pointer because of OnceLock.
    assert_eq!(a.as_ptr(), b.as_ptr());
}
