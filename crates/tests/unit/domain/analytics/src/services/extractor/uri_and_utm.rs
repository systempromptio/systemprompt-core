//! Tests for URI extraction, UTM parameters, referrer handling, socket addr, and locale edge cases.

use axum::http::{HeaderMap, HeaderValue, Uri};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use systemprompt_analytics::SessionAnalytics;

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
fn referrer_source_skips_ip_addresses() {
    let mut headers = HeaderMap::new();
    headers.insert("referer", HeaderValue::from_static("http://192.168.1.1/page"));
    let analytics = SessionAnalytics::from_headers(&headers);

    assert!(analytics.referrer_source.is_none());
}

#[test]
fn from_headers_with_socket_addr_fallback() {
    let headers = HeaderMap::new();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 100)), 8080);
    let analytics =
        SessionAnalytics::from_headers_with_geoip_and_socket(&headers, None, Some(socket));

    assert_eq!(analytics.ip_address, Some("192.168.0.100".to_string()));
}

#[test]
fn socket_addr_not_used_when_forwarded_for_present() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 100)), 8080);
    let analytics =
        SessionAnalytics::from_headers_with_geoip_and_socket(&headers, None, Some(socket));

    assert_eq!(analytics.ip_address, Some("10.0.0.1".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_utm_source() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_source=google".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);

    assert_eq!(analytics.utm_source, Some("google".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_utm_medium() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_medium=cpc".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);

    assert_eq!(analytics.utm_medium, Some("cpc".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_utm_campaign() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_campaign=summer_sale"
        .parse()
        .unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);

    assert_eq!(analytics.utm_campaign, Some("summer_sale".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_all_utm_params() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/?utm_source=google&utm_medium=cpc&utm_campaign=test"
        .parse()
        .unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);

    assert_eq!(analytics.utm_source, Some("google".to_string()));
    assert_eq!(analytics.utm_medium, Some("cpc".to_string()));
    assert_eq!(analytics.utm_campaign, Some("test".to_string()));
}

#[test]
fn from_headers_and_uri_without_uri() {
    let headers = create_full_headers();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, None, None, None);

    assert!(analytics.utm_source.is_none());
    assert!(analytics.entry_url.is_none());
    assert!(analytics.landing_page.is_none());
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
fn socket_addr_v6() {
    use std::net::Ipv6Addr;
    let headers = HeaderMap::new();
    let socket =
        SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8080);
    let analytics =
        SessionAnalytics::from_headers_with_geoip_and_socket(&headers, None, Some(socket));

    assert_eq!(analytics.ip_address, Some("::1".to_string()));
}
