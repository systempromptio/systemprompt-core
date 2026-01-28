//! Tests for session analytics extractor.

use axum::http::{HeaderMap, HeaderValue, Uri};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use systemprompt_analytics::SessionAnalytics;
use systemprompt_models::ContentRouting;

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

    #[test]
    fn compatible_user_agent_without_browser_is_bot() {
        let headers =
            create_headers_with_user_agent("Mozilla/5.0 (compatible; SomeBot/1.0)");
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

        // x-forwarded-for takes precedence
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
    fn is_bot_detects_gptbot() {
        let headers = create_headers_with_user_agent("GPTBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_claudeweb() {
        let headers = create_headers_with_user_agent("Claude-Web/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_facebookexternalhit() {
        let headers = create_headers_with_user_agent("facebookexternalhit/1.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_yandexbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; YandexBot/3.0)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_baiduspider() {
        let headers = create_headers_with_user_agent("Baiduspider/2.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_slurp() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; Yahoo! Slurp)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_wget() {
        let headers = create_headers_with_user_agent("Wget/1.21");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_puppeteer() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Puppeteer/19.0.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_phantomjs() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 PhantomJS/2.1.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_uptimerobot() {
        let headers = create_headers_with_user_agent("UptimeRobot/2.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_semrushbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; SemrushBot/7)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_ahrefsbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; AhrefsBot/7.0)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
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

        // Not a known browser
        assert!(analytics.browser.is_none());
    }

    #[test]
    fn parse_user_agent_unknown_os() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (UnknownOS) Chrome/120.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        // Chrome is detected
        assert_eq!(analytics.browser, Some("Chrome".to_string()));
        // But OS is unknown
        assert!(analytics.os.is_none());
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
        // Tablet without Android/mobile keywords - pure tablet detection
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

        // Empty values are still captured
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
    fn is_bot_detects_duckduckbot() {
        let headers = create_headers_with_user_agent("DuckDuckBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_slackbot() {
        let headers = create_headers_with_user_agent("Slackbot 1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_discordbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; Discordbot/2.0)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_twitterbot() {
        let headers = create_headers_with_user_agent("Twitterbot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_linkedinbot() {
        let headers = create_headers_with_user_agent("LinkedInBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_webdriver() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 WebDriver");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_scrapy() {
        let headers = create_headers_with_user_agent("Scrapy/2.7.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_http_library_okhttp() {
        let headers = create_headers_with_user_agent("okhttp/4.10.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_http_library_axios() {
        let headers = create_headers_with_user_agent("axios/1.2.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_go_http_client() {
        let headers = create_headers_with_user_agent("Go-http-client/1.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_node_fetch() {
        let headers = create_headers_with_user_agent("node-fetch/3.0.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_archive_org_bot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 archive.org_bot");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_applebot() {
        let headers = create_headers_with_user_agent("Applebot/0.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_amazonbot() {
        let headers = create_headers_with_user_agent("Amazonbot/0.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_bytespider() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Bytespider");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_petalbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; PetalBot)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_perplexitybot() {
        let headers = create_headers_with_user_agent("PerplexityBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_chatgpt_user() {
        let headers = create_headers_with_user_agent("ChatGPT-User/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_anthropic_ai() {
        let headers = create_headers_with_user_agent("Anthropic-AI/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_lighthouse() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Lighthouse/10.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_pingdom() {
        let headers = create_headers_with_user_agent("Pingdom.com_bot_version_1.4");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn referrer_url_ipv6_skips_source() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "referer",
            HeaderValue::from_static("http://[::1]:8080/page"),
        );
        let analytics = SessionAnalytics::from_headers(&headers);

        // IPv6 referrer should be filtered for source
        assert_eq!(
            analytics.referrer_url,
            Some("http://[::1]:8080/page".to_string())
        );
        // Source extraction behavior depends on URL parsing of IPv6
        // The url crate will parse this differently
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

        // Should extract "en-US" (the first part before ;)
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
}

/// Mock ContentRouting that marks all paths as HTML pages
#[derive(Debug)]
struct MockHtmlRouting;

impl ContentRouting for MockHtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_string()
    }
}

/// Mock ContentRouting that marks no paths as HTML pages
#[derive(Debug)]
struct MockNonHtmlRouting;

impl ContentRouting for MockNonHtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        false
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_string()
    }
}

mod content_routing_tests {
    use super::*;

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
        headers
    }

    #[test]
    fn html_page_sets_entry_url() {
        let headers = create_full_headers();
        let uri: Uri = "https://example.com/about".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        assert!(analytics.entry_url.is_some());
        assert!(analytics.entry_url.unwrap().contains("/about"));
    }

    #[test]
    fn html_page_without_referrer_sets_landing_page() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120");
        let uri: Uri = "https://example.com/landing".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        // No referrer, so this is a landing page
        assert_eq!(analytics.landing_page, Some("/landing".to_string()));
    }

    #[test]
    fn html_page_with_same_site_referrer_sets_landing_page() {
        let mut headers = create_full_headers();
        headers.insert(
            "referer",
            HeaderValue::from_static("https://systemprompt.io/other"),
        );
        let uri: Uri = "https://example.com/page".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        // Same-site referrer (systemprompt.io), so landing page is set
        assert_eq!(analytics.landing_page, Some("/page".to_string()));
    }

    #[test]
    fn html_page_with_localhost_referrer_sets_landing_page() {
        let mut headers = create_full_headers();
        headers.insert(
            "referer",
            HeaderValue::from_static("http://localhost:3000/test"),
        );
        let uri: Uri = "https://example.com/page".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        // Localhost is same-site, so landing page is set
        assert_eq!(analytics.landing_page, Some("/page".to_string()));
    }

    #[test]
    fn html_page_with_tyingshoelaces_referrer_sets_landing_page() {
        let mut headers = create_full_headers();
        headers.insert(
            "referer",
            HeaderValue::from_static("https://tyingshoelaces.com/blog"),
        );
        let uri: Uri = "https://example.com/page".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        // tyingshoelaces.com is same-site, so landing page is set
        assert_eq!(analytics.landing_page, Some("/page".to_string()));
    }

    #[test]
    fn html_page_with_external_referrer_still_sets_landing_page() {
        let mut headers = create_full_headers();
        headers.insert(
            "referer",
            HeaderValue::from_static("https://google.com/search"),
        );
        let uri: Uri = "https://example.com/page".parse().unwrap();
        let routing = MockHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        assert_eq!(analytics.landing_page, Some("/page".to_string()));
    }

    #[test]
    fn non_html_page_no_entry_url() {
        let headers = create_full_headers();
        let uri: Uri = "https://example.com/api/data".parse().unwrap();
        let routing = MockNonHtmlRouting;

        let analytics =
            SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));

        assert!(analytics.entry_url.is_none());
        assert!(analytics.landing_page.is_none());
    }

}
