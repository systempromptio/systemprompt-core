//! Tests for browser detection (continued) and OS detection.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

mod session_analytics_tests {
    use super::*;

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
}
