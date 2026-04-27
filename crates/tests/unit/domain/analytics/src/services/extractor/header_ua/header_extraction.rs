//! Tests for header extraction: user agent, IP, fingerprint, locale, referrer.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

use super::{create_headers_with_ip, create_headers_with_user_agent};

#[test]
fn from_headers_extracts_user_agent() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(
        analytics
            .user_agent
            .as_ref()
            .expect("user_agent should be present")
            .contains("Chrome")
    );
}

#[test]
fn from_headers_extracts_ip_from_forwarded_for() {
    let headers = create_headers_with_ip("10.0.0.1, 192.168.1.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(
        analytics.ip_address.expect("ip_address should be present"),
        "10.0.0.1"
    );
}

#[test]
fn from_headers_extracts_first_ip_from_chain() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("1.1.1.1, 2.2.2.2, 3.3.3.3"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.ip_address, Some("1.1.1.1".to_string()));
}

#[test]
fn from_headers_falls_back_to_x_real_ip() {
    let mut headers = HeaderMap::new();
    headers.insert("x-real-ip", HeaderValue::from_static("8.8.8.8"));
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.ip_address, Some("8.8.8.8".to_string()));
}

#[test]
fn from_headers_extracts_fingerprint() {
    let mut headers = HeaderMap::new();
    headers.insert("x-fingerprint", HeaderValue::from_static("fp_test_hash"));
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.fingerprint_hash, Some("fp_test_hash".to_string()));
}

#[test]
fn from_headers_extracts_preferred_locale() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept-language",
        HeaderValue::from_static("en-US,en;q=0.9,fr;q=0.8"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.preferred_locale, Some("en-US".to_string()));
}

#[test]
fn from_headers_extracts_locale_without_quality() {
    let mut headers = HeaderMap::new();
    headers.insert("accept-language", HeaderValue::from_static("fr-FR"));
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.preferred_locale, Some("fr-FR".to_string()));
}

#[test]
fn from_headers_extracts_referrer_url() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://example.com/page"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(
        analytics.referrer_url,
        Some("https://example.com/page".to_string())
    );
}

#[test]
fn from_headers_extracts_referrer_source() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://google.com/search"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.referrer_source, Some("google.com".to_string()));
}

#[test]
fn from_headers_handles_missing_headers() {
    let headers = HeaderMap::new();
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.user_agent.is_none());
    assert!(analytics.ip_address.is_none());
    assert!(analytics.fingerprint_hash.is_none());
    assert!(analytics.preferred_locale.is_none());
}

#[test]
fn parse_user_agent_detects_desktop() {
    let headers =
        create_headers_with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("desktop".to_string()));
}

#[test]
fn parse_user_agent_detects_mobile() {
    let headers =
        create_headers_with_user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS) Mobile Safari");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("mobile".to_string()));
}

#[test]
fn parse_user_agent_detects_tablet() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (iPad; CPU OS) Safari");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("tablet".to_string()));
}

#[test]
fn parse_user_agent_detects_android_mobile() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (Linux; Android 13; Mobile)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("mobile".to_string()));
}
