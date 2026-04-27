//! Tests for referrer edge cases, UA parsing edge cases, and URI handling.

use axum::http::{HeaderMap, HeaderValue, Uri};
use systemprompt_analytics::SessionAnalytics;

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

fn create_full_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
    headers.insert("x-fingerprint", HeaderValue::from_static("fp_abc123"));
    headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "referer",
        HeaderValue::from_static("https://google.com/search?q=test"),
    );
    headers
}

#[test]
fn referrer_source_extracts_subdomain_host() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://blog.example.com/article"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.referrer_source, Some("blog.example.com".to_string()));
}

#[test]
fn referrer_url_invalid_skips_source() {
    let mut headers = HeaderMap::new();
    headers.insert("referer", HeaderValue::from_static("not-a-valid-url"));
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.referrer_url, Some("not-a-valid-url".to_string()));
    assert!(analytics.referrer_source.is_none());
}

#[test]
fn parse_user_agent_unknown_browser() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (X11; Unknown) SomeBrowser/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.browser, Some("Other".to_string()));
}

#[test]
fn parse_user_agent_unknown_os() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (UnknownOS) Chrome/120.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.browser, Some("Chrome".to_string()));
    assert_eq!(analytics.os, Some("Other".to_string()));
}

#[test]
fn is_bot_compatible_with_firefox_is_not_bot() {
    let headers = create_headers_with_user_agent(
        "Mozilla/5.0 (compatible; Some; Firefox/121.0)",
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(!analytics.is_bot());
}

#[test]
fn is_bot_compatible_with_safari_is_not_bot() {
    let headers = create_headers_with_user_agent(
        "Mozilla/5.0 (compatible; Some; Safari/605.1)",
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(!analytics.is_bot());
}

#[test]
fn is_bot_compatible_with_edge_is_not_bot() {
    let headers = create_headers_with_user_agent(
        "Mozilla/5.0 (compatible; Some; Edge/120)",
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(!analytics.is_bot());
}

#[test]
fn parse_user_agent_detects_macos_keyword() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (Macintosh; macOS 14.0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.os, Some("macOS".to_string()));
}

#[test]
fn parse_user_agent_detects_ios_keyword() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (iOS 17.0) Safari/605");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.os, Some("iOS".to_string()));
}

#[test]
fn parse_user_agent_detects_ipad_as_tablet() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (iPad; CPU OS 17_0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("tablet".to_string()));
}

#[test]
fn parse_user_agent_tablet_keyword() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (Windows; Tablet; Chrome/120)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.device_type, Some("tablet".to_string()));
}

#[test]
fn from_headers_and_uri_with_no_query_string() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert!(analytics.utm_source.is_none());
    assert!(analytics.utm_medium.is_none());
    assert!(analytics.utm_campaign.is_none());
}

#[test]
fn from_headers_and_uri_with_empty_query_values() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_source=&utm_medium=".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_source, Some("".to_string()));
    assert_eq!(analytics.utm_medium, Some("".to_string()));
}

#[test]
fn from_headers_and_uri_with_mixed_query_params() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/?foo=bar&utm_source=newsletter&baz=qux"
        .parse()
        .unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_source, Some("newsletter".to_string()));
    assert!(analytics.utm_medium.is_none());
}

#[test]
fn referrer_url_ipv6_skips_source() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "referer",
        HeaderValue::from_static("http://[::1]:8080/page"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(
        analytics.referrer_url,
        Some("http://[::1]:8080/page".to_string())
    );
}

#[test]
fn ip_address_trims_whitespace() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("  192.168.1.1  , 10.0.0.1"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.ip_address, Some("192.168.1.1".to_string()));
}

#[test]
fn accept_language_handles_complex_quality() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept-language",
        HeaderValue::from_static("en-US;q=0.9,en;q=0.8,fr-CA;q=0.7"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.preferred_locale, Some("en-US".to_string()));
}

#[test]
fn locale_extraction_with_semicolon_in_first_value() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "accept-language",
        HeaderValue::from_static("fr-FR;q=1.0, en-US;q=0.5"),
    );
    let analytics = SessionAnalytics::from_headers(&headers);
    assert_eq!(analytics.preferred_locale, Some("fr-FR".to_string()));
}

#[test]
fn ai_crawler_chatgpt_user_is_classified_as_ai_crawler_not_bot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 ChatGPT-User/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_ai_crawler());
    assert!(!analytics.is_bot());
    assert!(!analytics.should_skip_tracking());
}

#[test]
fn ai_crawler_claudebot_is_classified_as_ai_crawler_not_bot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; ClaudeBot/1.0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_ai_crawler());
    assert!(!analytics.is_bot());
}

#[test]
fn ai_crawler_notebooklm_classified_correctly() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; Google-NotebookLM)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_ai_crawler());
    assert!(!analytics.is_bot());
}

#[test]
fn malformed_user_agent_template_string_is_bot() {
    let headers = create_headers_with_user_agent("{USER_AGENT}");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn malformed_user_agent_dash_is_bot() {
    let headers = create_headers_with_user_agent("-");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn malformed_user_agent_curly_braces_is_bot() {
    let headers = create_headers_with_user_agent("{some_template_var}");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn malformed_user_agent_null_literal_is_bot() {
    let headers = create_headers_with_user_agent("null");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn socket_addr_v6() {
    use std::net::{IpAddr, Ipv6Addr, SocketAddr};
    let headers = HeaderMap::new();
    let socket =
        SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8080);
    let analytics =
        SessionAnalytics::from_headers_with_geoip_and_socket(&headers, None, Some(socket));
    assert_eq!(analytics.ip_address, Some("::1".to_string()));
}
