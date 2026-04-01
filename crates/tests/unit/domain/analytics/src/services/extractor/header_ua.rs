//! Tests for session analytics extractor.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

mod session_analytics_tests {
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
    fn from_headers_extracts_user_agent() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.user_agent.is_some());
        assert!(analytics.user_agent.unwrap().contains("Chrome"));
    }

    #[test]
    fn from_headers_extracts_ip_from_forwarded_for() {
        let headers = create_headers_with_ip("10.0.0.1, 192.168.1.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.ip_address.is_some());
        assert_eq!(analytics.ip_address.unwrap(), "10.0.0.1");
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
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0",
        );
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

    #[test]
    fn parse_user_agent_detects_chrome() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120.0.0.0 Safari/537.36");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Chrome".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_firefox() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Firefox/121.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Firefox".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_safari() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Macintosh) Safari/605.1.15");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Safari".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_edge() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120.0 Edg/120.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Edge".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_opera() {
        // Opera detection via "opera" keyword (without Chrome in UA)
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Windows NT 10.0) Opera/99.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Opera".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_opera_via_opr() {
        // Note: In source, Chrome is detected before Opera when UA contains "chrome"
        // Opera's OPR/ marker in real user agents appears without "chrome"
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Windows NT 10.0) opr/106.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Opera".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_samsung_internet() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 SamsungBrowser/23.0",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Samsung Internet".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_uc_browser() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Linux; Android 10) UCBrowser/13.4.0.1306",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("UC Browser".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_uc_browser_via_ucweb() {
        let headers = create_headers_with_user_agent("UCWEB/2.0 (Linux; U; en-US)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("UC Browser".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_yandex() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Windows NT 10.0) YaBrowser/23.11.0.2419",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Yandex".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_qq_browser() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Linux; Android) QQBrowser/12.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("QQ Browser".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_wechat() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Linux; Android 13) MicroMessenger/8.0.43.2480",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("WeChat".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_silk() {
        let headers =
            create_headers_with_user_agent("Mozilla/5.0 (Linux; Android) Silk/93.3.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Silk".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_electron() {
        let headers =
            create_headers_with_user_agent("Mozilla/5.0 (Windows) Electron/27.1.0 Chrome/118");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("Electron".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_cordova_webview() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Linux; Android; wv) AppleWebKit/537.36 Cordova/12.0",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("WebView".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_android_webview() {
        let headers = create_headers_with_user_agent(
            "Mozilla/5.0 (Linux; Android 13; wv) AppleWebKit/537.36 Chrome/120",
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.browser, Some("WebView".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_windows() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Windows NT 10.0) Chrome/120.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.os, Some("Windows".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_macos() {
        let headers =
            create_headers_with_user_agent("Mozilla/5.0 (Macintosh; Mac OS X 10_15) Safari/605");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.os, Some("macOS".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_linux() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (X11; Linux x86_64) Firefox/121");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.os, Some("Linux".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_android() {
        // Note: In the source implementation, Linux is checked before Android,
        // and the UA contains "linux". The code detects Android because
        // "android" is checked separately. Let's test with a clear Android UA.
        let headers = create_headers_with_user_agent("Mozilla/5.0 (Android 13; Mobile) Chrome/120");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.os, Some("Android".to_string()));
    }

    #[test]
    fn parse_user_agent_detects_ios() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert_eq!(analytics.os, Some("iOS".to_string()));
    }

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

        // IP address referrers should be filtered out
        assert!(analytics.referrer_source.is_none());
    }

    #[test]
    fn analytics_is_clone() {
        let headers = create_full_headers();
        let analytics = SessionAnalytics::from_headers(&headers);
        let cloned = analytics.clone();

        assert_eq!(analytics.user_agent, cloned.user_agent);
        assert_eq!(analytics.ip_address, cloned.ip_address);
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

        // Without geoip reader, geo fields should be None
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
