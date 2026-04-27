//! Bot detection for compatible user agents, IP-based bot detection, and UTM
//! extraction tests.

use axum::http::{HeaderMap, HeaderValue, Uri};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use systemprompt_analytics::SessionAnalytics;

mod bot_ip_utm_tests {
    use super::*;

    fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
        headers
    }

    fn create_headers_with_ip(ip: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(ip).unwrap());
        headers
    }

    fn create_full_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/120.0.0.0 Safari/537.36",
            ),
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.1"));
        headers.insert("x-fingerprint", HeaderValue::from_static("abc123"));
        headers.insert(
            "accept-language",
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert(
            "referer",
            HeaderValue::from_static("https://google.com/search?q=test"),
        );
        headers
    }

    #[test]
    fn compatible_user_agent_without_browser_is_bot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; SomeBot/1.0)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn compatible_user_agent_with_chrome_is_not_bot() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT; Chrome/120.0)",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(!analytics.is_bot());
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
    fn is_bot_ip_returns_true_for_microsoft_157_ip() {
        let headers = create_headers_with_ip("157.55.39.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_true_for_microsoft_207_ip() {
        let headers = create_headers_with_ip("207.46.13.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_true_for_facebook_69_ip() {
        let headers = create_headers_with_ip("69.171.250.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_true_for_facebook_173_ip() {
        let headers = create_headers_with_ip("173.252.88.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_true_for_facebook_31_ip() {
        let headers = create_headers_with_ip("31.13.24.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn from_headers_and_uri_extracts_utm_source() {
        let headers = create_full_headers();
        let uri: Uri = "https://example.com/page?utm_source=google"
            .parse()
            .unwrap();
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
}
