//! Tests for bot detection and miscellaneous analytics.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

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

mod session_analytics_tests {
    use super::*;

    #[test]
    fn is_bot_returns_true_for_googlebot() {
        let headers = create_headers_with_user_agent("Googlebot/2.1");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_bingbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; bingbot/2.0)");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_crawler() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 SomeCrawler/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_spider() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 spider-bot");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_curl() {
        let headers = create_headers_with_user_agent("curl/7.68.0");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_python_requests() {
        let headers = create_headers_with_user_agent("python-requests/2.28.0");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_headless() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 HeadlessChrome/120.0");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_selenium() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Selenium/4.0");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_true_for_short_user_agent() {
        let headers = create_headers_with_user_agent("test");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_false_for_chrome() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0",
        );
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(!analytics.is_bot());
    }

    #[test]
    fn is_bot_returns_false_for_firefox() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
        );
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(!analytics.is_bot());
    }

    #[test]
    fn is_bot_ip_returns_true_for_googlebot_ip() {
        let headers = create_headers_with_ip("66.249.64.1");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_true_for_bing_ip() {
        let headers = create_headers_with_ip("40.77.167.1");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_ip_returns_false_for_regular_ip() {
        let headers = create_headers_with_ip("192.168.1.1");
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(!analytics.is_bot_ip());
    }

    #[test]
    fn is_bot_returns_true_when_no_user_agent() {
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_ip_returns_false_when_no_ip() {
        let headers = HeaderMap::new();
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(!analytics.is_bot_ip());
    }

    #[test]
    fn referrer_source_skips_ip_addresses() {
        let mut headers = HeaderMap::new();
        headers.insert("referer", HeaderValue::from_static("http://192.168.1.1/page"));
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.referrer_source.is_none());
    }

    #[test]
    fn analytics_is_debug() {
        let headers = create_full_headers();
        let analytics = SessionAnalytics::from_headers(&headers);
        let debug_str = format!("{:?}", analytics);
        assert!(debug_str.contains("SessionAnalytics"));
    }

    #[test]
    fn from_headers_with_geoip_without_reader() {
        let headers = create_full_headers();
        let analytics = SessionAnalytics::from_headers_with_geoip(&headers, None);
        assert!(analytics.country.is_none());
        assert!(analytics.region.is_none());
        assert!(analytics.city.is_none());
    }

    #[test]
    fn landing_page_and_entry_url_are_none_without_uri() {
        let headers = create_full_headers();
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.landing_page.is_none());
        assert!(analytics.entry_url.is_none());
    }

    #[test]
    fn utm_fields_are_none_without_uri() {
        let headers = create_full_headers();
        let analytics = SessionAnalytics::from_headers(&headers);
        assert!(analytics.utm_source.is_none());
        assert!(analytics.utm_medium.is_none());
        assert!(analytics.utm_campaign.is_none());
    }
}
